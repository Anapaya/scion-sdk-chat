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
//! `ScionTransport` end to end: `chat-server` reached over a simulated SCION network.
//!
//! Two ASes: the server in one, the client in the other.

use std::{net::SocketAddr, path::Path, time::Duration};

use chat_client_core::{
    ChatClient, ClientConfig, PollConfig, SnapToken, TransportKind, config::ScionConfig,
};
use chat_server::{cert, config::Transport, scion};
use pocketscion::util::{
    dev_auth_token,
    topologies::{IA132, IA212, PsSetup, UnderlayType, minimal::minimal_topology},
};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

/// How long the server is given to start reading from its socket.
///
/// The socket is bound before the endpoint reads from it, so the first attempts can be dropped
/// without anything being wrong.
const READY_TIMEOUT: Duration = Duration::from_secs(20);

/// A network, a server in it, and a client that can reach the server.
struct Fixture {
    client: ChatClient,
    _network: PsSetup,
    _data: TempDir,
    shutdown: CancellationToken,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

/// Starts two ASes, puts the server in one, and returns a client attached to the other.
async fn fixture() -> Fixture {
    let network = minimal_topology(UnderlayType::Snap).await;
    let client_api = network
        .endhost_api(IA132)
        .expect("the client's endhost api");
    let server_api = network
        .endhost_api(IA212)
        .expect("the server's endhost api");

    let data = TempDir::new().expect("a temporary directory");
    let auth_token_file = data.path().join("snap.token");
    std::fs::write(&auth_token_file, dev_auth_token()).expect("the token is written");

    // Made before the server starts, so the path the client pins is the one the server presents.
    let certificate = cert::ServerCert::load_or_create(data.path()).expect("a certificate");

    let shutdown = CancellationToken::new();
    let (host, port) = serve(
        data.path(),
        server_api.as_ref(),
        &auth_token_file,
        &shutdown,
    )
    .await;

    let client = ChatClient::new(ClientConfig {
        transport: TransportKind::Scion(ScionConfig {
            endhost_api: client_api.to_string().parse().expect("an endhost api"),
            snap_token: Some(SnapToken::new(dev_auth_token())),
            // This topology has no TSAR records.
            target: Some(host),
            cert_path: Some(certificate.cert_path.clone()),
        }),
        server_url: format!("https://{}:{port}", cert::SERVER_NAME)
            .parse()
            .expect("a server url"),
        poll: PollConfig::default(),
    })
    .await
    .expect("a client");

    await_ready(&client).await;

    Fixture {
        client,
        _network: network,
        _data: data,
        shutdown,
    }
}

/// Brings the server up in `IA212`, and answers with the SCION host and port it landed on.
async fn serve(
    data_dir: &Path,
    endhost_api: &str,
    auth_token_file: &Path,
    shutdown: &CancellationToken,
) -> (String, u16) {
    let config = chat_server::config::Config {
        transport: Transport::Scion,
        listen: SocketAddr::from(([127, 0, 0, 1], 0)),
        data_dir: data_dir.to_owned(),
        max_accounts: 500,
        max_rooms: 100,
        max_message_bytes: 4096,
        token_expiry_days: 7,
        endhost_api: Some(endhost_api.to_owned()),
        auth_token_file: Some(auth_token_file.to_owned()),
    };

    // Bound before it is served, which is the only way to learn the port it was given: serving
    // consumes the listener.
    let stack = scion::build_stack(&config).await.expect("a scion stack");
    let listener = scion::ScionListener::bind(stack, &config)
        .await
        .expect("a bound socket");
    let addr = listener.addr();
    let reachable = (addr.host().to_string(), addr.port());

    let state = chat_server::state(&config).await.expect("server state");
    let router = chat_server::api::router(state);

    tokio::spawn({
        let shutdown = shutdown.clone();
        async move {
            let _ = scion::serve_on(listener, &config, router, shutdown).await;
        }
    });

    reachable
}

async fn await_ready(client: &ChatClient) {
    let deadline = tokio::time::Instant::now() + READY_TIMEOUT;
    let mut last = None;

    while tokio::time::Instant::now() < deadline {
        match client.health().await {
            Ok(()) => return,
            Err(error) => last = Some(error),
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    panic!("the server never answered over SCION; last failure: {last:?}");
}

/// The acceptance criterion: everything the client does works with a SCION network under it, and
/// what was written in one AS reads back from another.
#[tokio::test(flavor = "multi_thread")]
async fn the_client_holds_a_conversation_over_scion() {
    let fixture = fixture().await;
    let client = &fixture.client;

    let info = client
        .server_info()
        .await
        .expect("the server describes itself");
    assert_eq!(info.max_message_bytes, 4096);

    client
        .register("ada", "a password")
        .await
        .expect("an account");
    client.login("ada", "a password").await.expect("a session");

    let room = client.create_room("general").await.expect("a room");
    client.send(room.id, "across the link").await.expect("sent");

    let seen: Vec<(String, String)> = client
        .messages_newest(room.id, 10)
        .await
        .expect("the room reads back")
        .into_iter()
        .map(|message| (message.username, message.body))
        .collect();

    assert_eq!(seen, [("ada".to_owned(), "across the link".to_owned())]);
}
