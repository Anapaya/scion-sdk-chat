// Copyright 2026 Anapaya Systems
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//! A SCION network on this machine, with the chat server in it.
//!
//! Two autonomous systems joined by one link: the server in `2-ff00:0:212`, a client in
//! `1-ff00:0:132`. The network lives for exactly as long as this process does.
//!
//! Nothing here belongs in a deployment. The simulated network is a dependency of this crate alone,
//! so the server binary cannot carry one.

pub mod config;
pub mod info;

use std::{
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{Router, extract::State, routing::get};
use chat_server::{cert, config::Transport, scion};
use pocketscion::{
    io_config::IoConfig,
    util::{
        dev_auth_token,
        topologies::{
            IA132, IA212, PsSetup, UnderlayType, minimal::minimal_topology_with_io_config,
        },
    },
};
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

pub use crate::{
    config::Config,
    info::{DevNetwork, Server},
};

/// What carries traffic between the two ASes.
///
/// SNAP addresses an endpoint at the address its tunnel observed. UDP addresses it at the one it
/// believes it has, which nothing behind a translation can reach.
const UNDERLAY: UnderlayType = UnderlayType::Snap;

/// Why a network could not be started.
#[derive(Debug, thiserror::Error)]
pub enum DevError {
    /// A directory or a file could not be written.
    #[error("{action} {path}")]
    Io {
        /// What was being done.
        action: &'static str,
        /// What it was being done to.
        path: PathBuf,
        /// The failure underneath.
        #[source]
        source: std::io::Error,
    },
    /// The topology reported no endhost API for one of its ASes.
    #[error("the topology has no endhost API for {isd_as}")]
    NoEndhostApi {
        /// The AS that was asked for.
        isd_as: String,
    },
    /// `--bind-ip` was a wildcard, which no SNAP tunnel can be dialled at.
    #[error(
        "--bind-ip cannot be a wildcard: the SNAP tunnel is dialled at this address, and \
         {0} names no host. Give this machine's own address, or keep 127.0.0.1 and use \
         --advertise-ip for a client that reaches it another way."
    )]
    WildcardBind(IpAddr),
    /// The certificate could not be read or written.
    #[error(transparent)]
    Cert(#[from] cert::CertError),
    /// The server could not be started.
    #[error(transparent)]
    Server(#[from] chat_server::RunError),
}

/// A running network, and the server in it when there is one.
///
/// Holding this is what keeps the network up: dropping it stops the topology, and with it every
/// address in the description.
pub struct DevSetup {
    /// Kept because the topology lives only as long as it does.
    _network: PsSetup,
    /// Kept because a temporary data directory is removed when it drops.
    _data: Option<TempDir>,
    control: TcpListener,
    network: DevNetwork,
    shutdown: CancellationToken,
}

impl DevSetup {
    /// Starts the network, and the server unless the caller asked for it to be left out.
    ///
    /// Binds the control listener before the topology, so a second copy of this process fails in
    /// milliseconds rather than after a whole network has started.
    pub async fn start(config: &Config) -> Result<Self, DevError> {
        if config.bind_ip.is_unspecified() {
            return Err(DevError::WildcardBind(config.bind_ip));
        }

        let control = TcpListener::bind(SocketAddr::new(config.bind_ip, config.control_port))
            .await
            .map_err(|source| {
                DevError::Io {
                    action: "binding the control API on",
                    path: PathBuf::from(format!("{}:{}", config.bind_ip, config.control_port)),
                    source,
                }
            })?;
        let control_url = format!("http://{}", local_addr(&control));

        scion_sdk_utils::rustls::select_ring_crypto_provider();

        let io = IoConfig::new();
        io.set_bind_ip(config.bind_ip);
        if let Some(ip) = config.advertise_ip {
            io.set_advertised_ip(IA132, ip);
        }
        let network = minimal_topology_with_io_config(UNDERLAY, io).await;

        let (data_dir, kept) = data_dir(config)?;
        let auth_token_file = data_dir.join("snap.token");
        write(&auth_token_file, dev_auth_token())?;

        // Made here so the description carries it even when the server is somebody else's process.
        let certificate = cert::load_or_create(&data_dir)?;
        let ca_pem = read(&certificate.cert_path)?;

        let client_api = endhost_api(network.endhost_api(IA132), IA132)?;
        let server_api = endhost_api(network.endhost_api(IA212), IA212)?;
        let listen = SocketAddr::new(config.bind_ip, config.server_port);

        let shutdown = CancellationToken::new();
        let served = if config.no_server {
            None
        } else {
            Some(start_server(&data_dir, listen, &server_api, &auth_token_file, &shutdown).await?)
        };

        // Under `--no-server` the address is predicted: on this topology a tunnel observes the
        // address it was told to bind.
        let (target, port) =
            served.unwrap_or_else(|| (format!("{},{}", IA212, listen.ip()), listen.port()));

        Ok(Self {
            control,
            network: DevNetwork {
                control_url,
                underlay: "snap".to_owned(),
                server: if config.no_server {
                    Server::External
                } else {
                    Server::Embedded
                },
                endhost_api_url: client_api,
                server_endhost_api_url: server_api.clone(),
                client_isd_as: IA132.to_string(),
                auth_token: dev_auth_token(),
                auth_token_file: auth_token_file.display().to_string(),
                base_url: format!("https://{}:{port}", cert::SERVER_NAME),
                target,
                ca_pem,
                ca_path: certificate.cert_path.display().to_string(),
                ca_fingerprint: certificate.fingerprint,
                data_dir: data_dir.display().to_string(),
                chat_server_args: info::chat_server_args(
                    &SocketAddr::new(listen.ip(), port).to_string(),
                    &data_dir,
                    &server_api,
                    &auth_token_file,
                ),
            },
            _network: network,
            _data: kept,
            shutdown,
        })
    }

    /// What a client needs to reach the server.
    pub fn network(&self) -> &DevNetwork {
        &self.network
    }

    /// A handle that stops the network, taken before [`DevSetup::serve`] consumes the setup.
    pub fn stopper(&self) -> CancellationToken {
        self.shutdown.clone()
    }

    /// Serves the description until the [`DevSetup::stopper`] is cancelled.
    ///
    /// Every read gets a token of its own. See [`DevNetwork::auth_token`] for why sharing one
    /// breaks the client that had it first.
    pub async fn serve(self) {
        let router = Router::new()
            .route(
                "/info",
                get(|State(network): State<Arc<DevNetwork>>| {
                    async move {
                        axum::Json(DevNetwork {
                            auth_token: dev_auth_token(),
                            ..(*network).clone()
                        })
                    }
                }),
            )
            .with_state(Arc::new(self.network));

        let shutdown = self.shutdown.clone();
        let served = axum::serve(self.control, router)
            .with_graceful_shutdown(async move { shutdown.cancelled().await })
            .await;
        if let Err(error) = served {
            tracing::error!(%error, "the control API stopped");
        }
    }
}

/// Starts the chat server, and answers with the SCION host and port it is reachable at.
async fn start_server(
    data_dir: &Path,
    listen: SocketAddr,
    endhost_api: &str,
    auth_token_file: &Path,
    shutdown: &CancellationToken,
) -> Result<(String, u16), DevError> {
    let config = chat_server::config::Config {
        transport: Transport::Scion,
        listen,
        data_dir: data_dir.to_owned(),
        max_accounts: 500,
        max_rooms: 100,
        max_message_bytes: 4096,
        token_expiry_days: 7,
        endhost_api: Some(endhost_api.to_owned()),
        auth_token_file: Some(auth_token_file.to_owned()),
    };

    // Bound before it is served so the address can be read: serving consumes the listener.
    let listener = scion::bind(&config).await?;
    let addr = listener.addr();
    let reachable = (addr.host().to_string(), addr.port());
    let state = chat_server::state(&config).await?;

    tokio::spawn({
        let shutdown = shutdown.clone();
        async move {
            let served =
                scion::serve_on(listener, &config, chat_server::api::router(state), shutdown).await;
            if let Err(error) = served {
                tracing::error!(%error, "the chat server stopped");
            }
        }
    });

    Ok(reachable)
}

/// Where to write, and the directory to hold onto when it is a temporary one.
///
/// A temporary one unless a directory is named, so a run starts with no accounts.
fn data_dir(config: &Config) -> Result<(PathBuf, Option<TempDir>), DevError> {
    match &config.data_dir {
        Some(named) => {
            std::fs::create_dir_all(named).map_err(|source| {
                DevError::Io {
                    action: "creating the data directory",
                    path: named.clone(),
                    source,
                }
            })?;

            Ok((named.clone(), None))
        }
        None => {
            let made = TempDir::new().map_err(|source| {
                DevError::Io {
                    action: "creating a temporary data directory",
                    path: PathBuf::from("a temporary directory"),
                    source,
                }
            })?;

            Ok((made.path().to_owned(), Some(made)))
        }
    }
}

fn endhost_api(found: Option<impl ToString>, isd_as: impl ToString) -> Result<String, DevError> {
    found.map(|url| url.to_string()).ok_or_else(|| {
        DevError::NoEndhostApi {
            isd_as: isd_as.to_string(),
        }
    })
}

fn local_addr(listener: &TcpListener) -> SocketAddr {
    listener
        .local_addr()
        .unwrap_or_else(|_| SocketAddr::new(IpAddr::from([127, 0, 0, 1]), 0))
}

fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), DevError> {
    std::fs::write(path, contents).map_err(|source| {
        DevError::Io {
            action: "writing",
            path: path.to_owned(),
            source,
        }
    })
}

fn read(path: &Path) -> Result<String, DevError> {
    std::fs::read_to_string(path).map_err(|source| {
        DevError::Io {
            action: "reading",
            path: path.to_owned(),
            source,
        }
    })
}
