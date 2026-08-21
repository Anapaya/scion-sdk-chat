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
//! Serving the API over HTTP/3 on SCION.
//!
//! The network is reached through an endhost API rather than configured here: it reports which
//! underlays are available and the stack opens a socket on one of them. Everything above this file
//! is an ordinary [`axum::Router`], which is the point — the transport is all that changes.
//!
//! Types come from `scion_stack`'s re-exports rather than from `sciparse` and `scion-quic`
//! directly, so there is one version of each in the build and no chance of handing a
//! `sciparse::Url` to something expecting another one.

use std::{fs, sync::Arc};

use axum::Router;
use scion_h3_axum::ScionH3AxumServer;
use scion_stack::{
    ScionStack,
    scion_quic::{quic::config::QuicConfig, reexport::squiche, socket::GenericScionUdpSocket},
    sciparse::address::ip_socket_addr::ScionSocketIpAddr,
    stack::ScionStackBuilder,
    url::Url,
};
use tokio_util::sync::CancellationToken;

use crate::{RunError, cert, config::Config};

/// A bound socket, and the stack that socket belongs to.
///
/// The stack is carried rather than dropped once the socket exists. It owns the background tasks
/// that keep paths fresh and the SNAP token renewed, and the SDK's rule is one stack for as long as
/// the process wants SCION connectivity — dropping it early leaves a socket that works until the
/// first path expires and then quietly stops.
pub struct Listener {
    /// Never read. Held so that its background tasks outlive binding.
    _stack: ScionStack,
    socket: Arc<dyn GenericScionUdpSocket>,
}

impl Listener {
    /// Where clients should send, once they know a path to this AS.
    pub fn addr(&self) -> ScionSocketIpAddr {
        self.socket.local_addr()
    }
}

/// Serves the API over HTTP/3-over-SCION until the process is asked to stop.
pub async fn serve(config: &Config, router: Router) -> Result<(), RunError> {
    serve_on(bind(config).await?, config, router, on_ctrl_c()).await
}

/// Serves on an already-bound [`Listener`], stopping when `shutdown` is cancelled.
///
/// Separate from [`serve`] because only the caller of [`bind`] can learn the address the socket
/// landed on, which a test needs before it can send anything.
pub async fn serve_on(
    listener: Listener,
    config: &Config,
    router: Router,
    shutdown: CancellationToken,
) -> Result<(), RunError> {
    let addr = listener.addr();

    let cert = cert::load_or_create(&config.data_dir)?;
    // The one line an operator has to pass on: clients pin this, and nothing else identifies the
    // server.
    tracing::info!(
        fingerprint = %cert.fingerprint,
        cert = %cert.cert_path.display(),
        "pin this certificate"
    );

    let quic = quic_config(&cert)?;
    tracing::info!(%addr, server_name = cert::SERVER_NAME, "serving over scion");

    ScionH3AxumServer::serve_with_graceful_shutdown(
        Arc::clone(&listener.socket),
        router,
        quic,
        shutdown,
    )
    .await
    .map_err(|source| {
        RunError::Scion {
            action: "serving over scion",
            detail: source.to_string(),
        }
    })
    // `listener` is dropped here, taking the stack with it — after serving has stopped, never
    // while it is running.
}

/// Builds the stack and opens the socket the server listens on.
pub async fn bind(config: &Config) -> Result<Listener, RunError> {
    // Before anything that speaks TLS, which endhost API discovery below is the first to do. rustls
    // refuses to build a configuration until a provider is installed, and the SDK installs none on
    // an application's behalf.
    scion_sdk_utils::rustls::select_ring_crypto_provider();

    let stack = build_stack(config).await?;

    // The endhost API decides which AS the host is in, so `--listen` contributes only its IP and
    // port. Binding explicitly rather than letting the stack choose is what makes the port
    // predictable, which is what lets a client be configured before the server starts.
    let isd_asn = *stack.local_ases().first().ok_or_else(|| {
        RunError::Scion {
            action: "reading the local AS",
            detail: "the endhost API reported no AS for this host".to_owned(),
        }
    })?;
    let bind_addr = ScionSocketIpAddr::new(isd_asn, config.listen.ip(), config.listen.port());

    let socket = stack.bind(Some(bind_addr)).await.map_err(|source| {
        RunError::Scion {
            action: "binding a SCION socket",
            detail: source.to_string(),
        }
    })?;

    Ok(Listener {
        _stack: stack,
        socket: Arc::new(socket),
    })
}

async fn build_stack(config: &Config) -> Result<ScionStack, RunError> {
    let endhost_api = config.endhost_api.as_deref().ok_or_else(|| {
        RunError::Config(
            "--endhost-api is required by --transport scion: it is how the server finds the \
             network. A local PocketSCION topology prints one at startup."
                .to_owned(),
        )
    })?;
    let endhost_api = Url::parse(endhost_api).map_err(|source| {
        RunError::Config(format!(
            "--endhost-api \"{endhost_api}\" is not a URL: {source}"
        ))
    })?;

    let mut builder = ScionStackBuilder::new().with_endhost_api(endhost_api);
    if let Some(path) = &config.auth_token_file {
        let token = fs::read_to_string(path).map_err(|source| {
            RunError::Config(format!(
                "could not read --auth-token-file {}: {source}",
                path.display()
            ))
        })?;
        builder = builder.with_auth_token(token.trim().to_owned());
    }

    builder.build().await.map_err(|source| {
        RunError::Scion {
            action: "building the SCION stack",
            detail: source.to_string(),
        }
    })
}

/// The QUIC configuration, carrying the certificate clients pin.
fn quic_config(cert: &cert::ServerCert) -> Result<squiche::Config, RunError> {
    let failed = |action: &'static str| {
        move |source: squiche::Error| {
            RunError::Scion {
                action,
                detail: source.to_string(),
            }
        }
    };
    let path = |file: &std::path::Path| -> Result<String, RunError> {
        file.to_str().map(str::to_owned).ok_or_else(|| {
            RunError::Scion {
                action: "reading the certificate path",
                detail: format!("{} is not valid UTF-8", file.display()),
            }
        })
    };

    let mut quic = QuicConfig::builder()
        .build()
        .to_quiche_config()
        .map_err(failed("building the QUIC configuration"))?;

    // Files rather than the bytes already in hand: squiche loads certificates through BoringSSL,
    // which reads them from disk and offers no in-memory equivalent.
    quic.load_cert_chain_from_pem_file(&path(&cert.cert_path)?)
        .map_err(failed("loading the certificate"))?;
    quic.load_priv_key_from_pem_file(&path(&cert.key_path)?)
        .map_err(failed("loading the private key"))?;

    Ok(quic)
}

/// A token that is cancelled when the process is interrupted.
fn on_ctrl_c() -> CancellationToken {
    let shutdown = CancellationToken::new();
    tokio::spawn({
        let shutdown = shutdown.clone();
        async move {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("shutting down");
            shutdown.cancel();
        }
    });

    shutdown
}
