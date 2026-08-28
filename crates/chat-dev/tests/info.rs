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
//! What a client does with this network: read the description, then use it.
//!
//! The description is fetched over plain TCP, as any client fetches it. Everything after that is
//! over SCION.

use std::{path::PathBuf, time::Duration};

use chat_client_core::{
    ChatClient, ClientConfig, PollConfig, SnapToken, TransportKind, config::ScionConfig,
};
use chat_dev::{Config, DevNetwork, DevSetup, Server};

/// How long the server is given to start reading from its socket.
const READY_TIMEOUT: Duration = Duration::from_secs(20);

/// A network on ports nothing else is using, so a run cannot collide with a running `chat-dev`.
fn ephemeral() -> Config {
    Config {
        control_port: 0,
        bind_ip: "127.0.0.1".parse().expect("an address"),
        advertise_ip: None,
        server_port: 0,
        data_dir: None,
        no_server: false,
    }
}

/// The description, as any client reads it: over plain TCP, without SCION.
async fn describe(control_url: &str) -> DevNetwork {
    reqwest::get(format!("{control_url}/info"))
        .await
        .expect("the control api answers")
        .json()
        .await
        .expect("a description")
}

/// Builds the client a description describes.
async fn client(network: &DevNetwork) -> ChatClient {
    ChatClient::new(ClientConfig {
        transport: TransportKind::Scion(ScionConfig {
            endhost_api: network.endhost_api_url.parse().expect("an endhost api"),
            snap_token: Some(SnapToken::new(network.auth_token.clone())),
            target: Some(network.target.clone()),
            cert_path: Some(PathBuf::from(&network.ca_path)),
        }),
        server_url: network.base_url.parse().expect("a server url"),
        poll: PollConfig::default(),
    })
    .await
    .expect("a client")
}

/// The socket is bound before the endpoint reads from it, so the first attempts can be dropped.
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

    panic!("the server never answered; last failure: {last:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_description_is_enough_to_reach_the_server() {
    let setup = DevSetup::start(&ephemeral()).await.expect("a network");
    let served = setup.network().clone();
    let stop = setup.stopper();
    let serving = tokio::spawn(setup.serve());

    // Read back over TCP, as a client that did not start this process would.
    let fetched = describe(&served.control_url).await;
    assert_eq!(
        DevNetwork {
            auth_token: served.auth_token.clone(),
            ..fetched.clone()
        },
        served,
        "what is served is what was printed, but for the token",
    );

    let client = client(&fetched).await;
    await_ready(&client).await;

    let info = client
        .server_info()
        .await
        .expect("the server describes itself");
    assert_eq!(info.max_message_bytes, 4096);

    stop.cancel();
    serving.await.expect("serving should not panic");
}

/// Two clients must not share a token. See [`DevNetwork::auth_token`].
#[tokio::test(flavor = "multi_thread")]
async fn every_reader_is_given_a_token_of_its_own() {
    let setup = DevSetup::start(&Config {
        no_server: true,
        ..ephemeral()
    })
    .await
    .expect("a network");
    let control_url = setup.network().control_url.clone();
    let server_token = setup.network().auth_token.clone();
    let stop = setup.stopper();
    let serving = tokio::spawn(setup.serve());

    let first = describe(&control_url).await;
    let second = describe(&control_url).await;

    assert_ne!(first.auth_token, second.auth_token);
    assert_ne!(
        first.auth_token, server_token,
        "the token printed at startup is spent too",
    );
    // Everything a client reads besides the token is the same answer every time.
    assert_eq!(first.endhost_api_url, second.endhost_api_url);
    assert_eq!(first.target, second.target);
    assert_eq!(first.ca_fingerprint, second.ca_fingerprint);

    stop.cancel();
    serving.await.expect("serving should not panic");
}

/// Two people in the room at once. One client proves the transport; two prove they do not evict
/// each other.
#[tokio::test(flavor = "multi_thread")]
async fn two_clients_hold_a_conversation_across_the_link() {
    let setup = DevSetup::start(&ephemeral()).await.expect("a network");
    let control_url = setup.network().control_url.clone();
    let stop = setup.stopper();
    let serving = tokio::spawn(setup.serve());

    let ada = client(&describe(&control_url).await).await;
    let grace = client(&describe(&control_url).await).await;
    await_ready(&ada).await;

    for (client, who) in [(&ada, "ada"), (&grace, "grace")] {
        client
            .register(who, "a password")
            .await
            .expect("an account");
        client.login(who, "a password").await.expect("a session");
    }

    let room = ada.create_room("general").await.expect("a room");
    ada.send(room.id, "from the first").await.expect("sent");
    grace.send(room.id, "from the second").await.expect("sent");

    // Read as the other one, the message crossed the link. Read as the first, its tunnel still
    // carries traffic.
    let expected = [
        ("ada".to_owned(), "from the first".to_owned()),
        ("grace".to_owned(), "from the second".to_owned()),
    ];
    for (client, who) in [(&grace, "grace"), (&ada, "ada")] {
        let seen: Vec<(String, String)> = client
            .messages_newest(room.id, 10)
            .await
            .expect("the room reads back")
            .into_iter()
            .map(|message| (message.username, message.body))
            .collect();

        assert_eq!(seen, expected, "as read by {who}");
    }

    stop.cancel();
    serving.await.expect("serving should not panic");
}

/// Refused before anything starts: the failure underneath arrives a whole topology later and says
/// only "error establishing SNAP tunnel".
#[tokio::test(flavor = "multi_thread")]
async fn a_wildcard_bind_is_refused_with_something_to_act_on() {
    let started = DevSetup::start(&Config {
        bind_ip: "0.0.0.0".parse().expect("an address"),
        ..ephemeral()
    })
    .await;

    let Err(error) = started else {
        panic!("a wildcard names no host the SNAP tunnel can be dialled at");
    };
    let said = error.to_string();
    assert!(said.contains("--bind-ip"), "{said}");
    assert!(said.contains("--advertise-ip"), "{said}");
}

/// Everything but the server, for a reader who is about to start one themselves.
#[tokio::test(flavor = "multi_thread")]
async fn a_network_without_a_server_still_says_how_to_join_it() {
    let setup = DevSetup::start(&Config {
        no_server: true,
        ..ephemeral()
    })
    .await
    .expect("a network");
    let network = setup.network();

    assert_eq!(network.server, Server::External);
    assert!(!network.endhost_api_url.is_empty());
    assert!(!network.server_endhost_api_url.is_empty());
    // The certificate is this process's to make, so it is described either way.
    assert!(network.ca_pem.contains("BEGIN CERTIFICATE"));
    assert!(
        network
            .chat_server_args
            .contains(&network.server_endhost_api_url),
        "the arguments point at the server's own AS: {:?}",
        network.chat_server_args
    );

    setup.stopper().cancel();
}
