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
//! The client is the real [`ChatClient`] on `--transport scion`, so this covers the transport a
//! user gets rather than a hand-rolled stand-in for it.

use std::{path::Path, time::Duration};

use chat_client_core::{
    ChatClient, ClientConfig, SnapToken, TransportKind,
    config::{PollConfig, ScionConfig},
};
use chat_server::{
    cert,
    config::{Config, Transport},
    scion,
};
use pocketscion::util::{
    dev_auth_token,
    topologies::{IA132, IA212, PsSetup, UnderlayType, minimal::minimal_topology},
};
use tokio_util::sync::CancellationToken;

/// Whatever port is free, read back from the listener. A fixed one would collide with a `chat-dev`
/// left running, which serves on 8443 by default.
const SERVER_PORT: u16 = 0;

/// How long the client keeps asking before the server is declared absent.
const READY_TIMEOUT: Duration = Duration::from_secs(20);

/// How long to wait between attempts while the endpoint is not yet reading.
const READY_INTERVAL: Duration = Duration::from_millis(250);

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
    let client = ChatClient::new(client_config(
        &network,
        &server_addr,
        &server_cert.cert_path,
    ))
    .await
    .expect("building the chat client");

    // The socket is bound before the task starts, but the QUIC endpoint only begins reading from it
    // inside `serve_on`, so a packet sent before that is simply dropped. Nothing reports readiness,
    // so the client asks until it is answered.
    await_ready(&client).await;

    // Health passing proves the round trip. Reading the server's own description then proves a body
    // survived it and decoded, which a status alone does not.
    let info = client
        .server_info()
        .await
        .expect("the server describes itself over scion");
    assert_eq!(
        info.max_message_bytes, config.max_message_bytes,
        "the limits came back from a different server than the test configured",
    );
    // Not asserted here: `info.isd_as`, which would say the answer came from the other AS in the
    // server's own words. `server_info` reports `None` whatever the transport. Crossing the link is
    // covered anyway — the client dials the address bound in `2-ff00:0:212` from `1-ff00:0:132`.

    shutdown.cancel();
    server
        .await
        .expect("the server task should not panic")
        .expect("the server should stop cleanly");
}

/// Waits until the server answers a health check, or fails the test saying it never did.
async fn await_ready(client: &ChatClient) {
    let deadline = tokio::time::Instant::now() + READY_TIMEOUT;
    let mut last = None;

    while tokio::time::Instant::now() < deadline {
        match client.health().await {
            Ok(()) => return,
            Err(error) => last = Some(error),
        }
        tokio::time::sleep(READY_INTERVAL).await;
    }

    panic!("the server never answered a health check; last failure: {last:?}");
}

fn server_config(data_dir: &Path, network: &PsSetup) -> Config {
    let token = data_dir.join("snap.token");
    std::fs::write(&token, dev_auth_token()).expect("writing the dev token");

    Config {
        transport: Transport::Scion,
        // The endhost API decides which AS the socket lands in, and on SNAP the host address is the
        // one the tunnel observes rather than the one asked for.
        listen: format!("127.0.0.1:{SERVER_PORT}")
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

/// A client in the other AS, trusting only the server's certificate.
///
/// The URL carries the certificate's own name rather than an address, because that name is the
/// identity the handshake checks. `target` is what saves the name from needing a TSAR record: the
/// simulation has no DNS, so the address is given instead of looked up.
fn client_config(
    network: &PsSetup,
    server_addr: &scion_stack::sciparse::address::ip_socket_addr::ScionSocketIpAddr,
    cert_path: &Path,
) -> ClientConfig {
    let server_url = format!("https://{}:{}", cert::SERVER_NAME, server_addr.port())
        .parse()
        .expect("a server url");

    ClientConfig {
        transport: TransportKind::Scion(ScionConfig {
            endhost_api: network
                .endhost_api(IA132)
                .expect("PocketSCION has an endhost API for the client AS"),
            snap_token: Some(SnapToken::new(dev_auth_token())),
            target: Some(server_addr.host().to_string()),
            cert_path: Some(cert_path.to_owned()),
        }),
        server_url,
        poll: PollConfig::default(),
    }
}
