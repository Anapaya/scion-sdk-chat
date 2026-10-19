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

use std::{fs, path::Path, sync::Arc};

use anapaya_aa_client::AnapayaAaTokenSource;
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

/// The device id the server sends to the AA.
const DEVICE_ID: &str = "chat-server";

/// A socket bound on a [`ScionStack`], and the stack it belongs to.
pub struct ScionListener {
    _stack: ScionStack,
    socket: Arc<dyn GenericScionUdpSocket>,
}

impl ScionListener {
    /// Opens the socket the server listens on, taking ownership of the stack it is bound to.
    pub async fn bind(stack: ScionStack, config: &Config) -> Result<Self, RunError> {
        // ANCHOR: bind
        // The endhost API decides which AS the host is in, so `--listen` contributes only its IP
        // and port. Binding explicitly is what makes the port predictable.
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
        // ANCHOR_END: bind

        Ok(Self {
            _stack: stack,
            socket: Arc::new(socket),
        })
    }

    /// Where clients should send, once they know a path to this AS.
    pub fn addr(&self) -> ScionSocketIpAddr {
        self.socket.local_addr()
    }
}

/// Serves the API over HTTP/3-over-SCION, stopping when `shutdown` is cancelled.
pub async fn serve(
    config: &Config,
    router: Router,
    shutdown: CancellationToken,
) -> Result<(), RunError> {
    let stack = build_stack(config).await?;
    let listener = ScionListener::bind(stack, config).await?;

    serve_on(listener, config, router, shutdown).await
}

/// Serves on an already-bound [`ScionListener`], stopping when `shutdown` is cancelled.
pub async fn serve_on(
    listener: ScionListener,
    config: &Config,
    router: Router,
    shutdown: CancellationToken,
) -> Result<(), RunError> {
    let addr = listener.addr();

    // ANCHOR: certificate
    let cert = cert::ServerCert::load_or_create(&config.data_dir)?;
    // The one line an operator has to pass on. Nothing else identifies the server.
    tracing::info!(
        fingerprint = %cert.fingerprint,
        cert = %cert.cert_path.display(),
        "pin this certificate"
    );
    // ANCHOR_END: certificate

    let quic = quic_config(&cert)?;
    tracing::info!(%addr, server_name = cert::SERVER_NAME, "serving over scion");

    // ANCHOR: serve
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
    // ANCHOR_END: serve
}

/// Builds the stack that reaches the SCION network.
pub async fn build_stack(config: &Config) -> Result<ScionStack, RunError> {
    // Before endhost API discovery below, which is the first thing here to speak TLS. The SDK
    // installs no provider on an application's behalf.
    scion_sdk_utils::rustls::select_ring_crypto_provider();

    let endhost_api = config.endhost_api.as_deref().ok_or_else(|| {
        RunError::Config(
            "--endhost-api is required by --transport scion: it is how the server finds the \
             network."
                .to_owned(),
        )
    })?;
    let endhost_api = Url::parse(endhost_api).map_err(|source| {
        RunError::Config(format!(
            "--endhost-api \"{endhost_api}\" is not a URL: {source}"
        ))
    })?;
    let aa_url = Url::parse(&config.aa_url).map_err(|source| {
        RunError::Config(format!(
            "--aa-url \"{}\" is not a URL: {source}",
            config.aa_url
        ))
    })?;

    // ANCHOR: build-stack
    let mut builder = ScionStackBuilder::new().with_endhost_api(endhost_api);
    if let Some(path) = &config.auth_token_file {
        builder = builder.with_auth_token(read_secret(path, "--auth-token-file")?);
    }
    if let Some(path) = &config.auth_api_key_file {
        let api_key = read_secret(path, "--auth-api-key-file")?;
        // The first exchange runs here, so a refused key stops the server with the AA's reason.
        let (source, _) = AnapayaAaTokenSource::start(api_key, &aa_url, DEVICE_ID).await?;
        builder = builder.with_auth_token_source(source);
    }

    builder.build().await.map_err(|source| {
        RunError::Scion {
            action: "building the SCION stack",
            detail: source.to_string(),
        }
    })
    // ANCHOR_END: build-stack
}

/// The secret in the file that `flag` names, without the line break an editor leaves at the end.
fn read_secret(path: &Path, flag: &str) -> Result<String, RunError> {
    let secret = fs::read_to_string(path).map_err(|source| {
        RunError::Config(format!(
            "could not read {flag} {}: {source}",
            path.display()
        ))
    })?;
    Ok(secret.trim().to_owned())
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

    quic.load_cert_chain_from_pem_file(&path(&cert.cert_path)?)
        .map_err(failed("loading the certificate"))?;
    quic.load_priv_key_from_pem_file(&path(&cert.key_path)?)
        .map_err(failed("loading the private key"))?;

    Ok(quic)
}
