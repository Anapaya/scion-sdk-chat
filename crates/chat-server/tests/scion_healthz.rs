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
//! The server answering over HTTP/3-over-SCION, on a PocketSCION network started by the test.
//!
//! Two ASes joined by one link: the server in `2-ff00:0:212`, the client in `1-ff00:0:132`. This is
//! what proves `--transport scion` end to end, since nothing below the axum router is exercised by
//! the TCP tests.
//!
//! The client here is deliberately the low-level [`Http3Client`] from `scion-quic`.
//!
//! @TODO: replace with scion-http3-client — it is the supported client surface, and it would remove
//! the socket, the QUIC config and the address plumbing below. It is not published yet.

use std::{path::Path, sync::Arc};

use chat_server::{
    cert,
    config::{Config, Transport},
    scion,
};
use http_body_util::BodyExt as _;
use pocketscion::util::{
    dev_auth_token,
    topologies::{IA132, IA212, UnderlayType, minimal::minimal_topology},
};
use scion_stack::{
    scion_quic::{
        h3::client::Http3Client, quic::config::QuicConfig, socket::GenericScionUdpSocket,
    },
    sciparse::address::ip_socket_addr::ScionSocketIpAddr,
    stack::ScionStackBuilder,
};
use tokio_util::sync::CancellationToken;

/// A port the server binds in its own AS, so the client can be pointed at it before it starts.
const SERVER_PORT: u16 = 8443;

#[tokio::test(flavor = "multi_thread")]
async fn healthz_answers_over_scion() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    let network = minimal_topology(UnderlayType::Snap).await;
    let data_dir = tempfile::tempdir().expect("a temp dir");

    // Generated here rather than waiting for the server to do it, because the client has to pin the
    // certificate before the first packet. Reading it twice is the point of persisting it.
    let server_cert = cert::load_or_create(data_dir.path()).expect("a certificate");

    let config = server_config(data_dir.path(), &network);
    let listener = scion::bind(&config)
        .await
        .expect("binding the server socket");
    let server_addr = listener.addr();

    let shutdown = CancellationToken::new();
    let server = tokio::spawn({
        let (config, shutdown) = (config.clone(), shutdown.clone());
        async move {
            let state = chat_server::state(&config).await.expect("server state");
            scion::serve_on(listener, &config, chat_server::api::router(state), shutdown).await
        }
    });

    // No signature-algorithm preference: the certificate is ECDSA P-256, which BoringSSL accepts by
    // default. An Ed25519 certificate would need one and still fail — see `cert::generate`.
    let client = client(&network, server_addr, &server_cert.cert_path).await;
    // The socket is bound before the task starts, but the QUIC endpoint only begins reading from it
    // inside `serve_on`, so a packet sent before that is simply dropped. Nothing reports readiness,
    // so the client asks until it is answered.
    await_ready(&client).await;

    let request = http::Request::builder()
        .method(http::Method::GET)
        .uri(format!("https://{}/api/v1/healthz", cert::SERVER_NAME))
        .body(())
        .expect("a request");

    // The two bodies have no ordering in HTTP/3, so the crate requires them driven concurrently.
    // A GET has nothing to send, and closing the write side is what puts the FIN on the wire.
    let (response, writer) = client.request(request).await.expect("issuing the request");
    tokio::spawn(async move { writer.finish().await });

    let response = response.await.expect("a response over scion");
    assert!(
        response.status().is_success(),
        "healthz answered {}",
        response.status()
    );

    let body = response
        .into_body()
        .collect()
        .await
        .expect("the response body")
        .to_bytes();
    let body = String::from_utf8_lossy(&body);
    assert!(
        body.contains("\"ok\""),
        "healthz said something unexpected: {body}"
    );

    shutdown.cancel();
    server
        .await
        .expect("the server task should not panic")
        .expect("the server should stop cleanly");
}

/// Waits until the server's endpoint answers a handshake, or fails the test saying it never did.
async fn await_ready(client: &Http3Client) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let mut last = None;

    while std::time::Instant::now() < deadline {
        match client.connect().await {
            Ok(()) => return,
            Err(error) => last = Some(error),
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }

    panic!("the server never completed a handshake; last failure: {last:?}");
}

fn server_config(data_dir: &Path, network: &pocketscion::util::topologies::PsSetup) -> Config {
    let token = data_dir.join("snap.token");
    std::fs::write(&token, dev_auth_token()).expect("writing the dev token");

    Config {
        transport: Transport::Scion,
        // Only the port is used on SCION: the endhost API decides which AS the socket lands in.
        listen: format!("0.0.0.0:{SERVER_PORT}")
            .parse()
            .expect("an address"),
        data_dir: data_dir.to_owned(),
        max_accounts: 10,
        max_rooms: 10,
        max_message_bytes: 4096,
        token_expiry_days: 1,
        endhost_api: Some(
            network
                .endhost_api(IA212)
                .expect("PocketSCION has an endhost API for the server AS")
                .to_string(),
        ),
        auth_token_file: Some(token),
    }
}

/// An HTTP/3 client in the other AS, trusting only the server's certificate.
async fn client(
    network: &pocketscion::util::topologies::PsSetup,
    server_addr: ScionSocketIpAddr,
    cert_path: &Path,
) -> Http3Client {
    let stack = ScionStackBuilder::new()
        .with_endhost_api(
            network
                .endhost_api(IA132)
                .expect("PocketSCION has an endhost API for the client AS"),
        )
        .with_auth_token(dev_auth_token())
        .build()
        .await
        .expect("building the client stack");
    let socket: Arc<dyn GenericScionUdpSocket> =
        Arc::new(stack.bind(None).await.expect("binding the client socket"));

    let quic = QuicConfig::builder()
        .ca_certs_file(cert_path.to_str().expect("a UTF-8 path"))
        .build();

    Http3Client::with_config(
        server_addr,
        socket,
        Some(cert::SERVER_NAME.to_owned()),
        quic,
    )
}
