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
//! What a client does with this network. It reads the description, then it uses it.
//!
//! Each test reads the description over plain TCP, as a client reads it. Every call after that
//! goes over SCION.

use std::{path::PathBuf, time::Duration};

use chat_client_core::{
    ChatClient, ClientConfig, PollConfig, SnapToken, TransportKind, config::ScionConfig,
};
use chat_dev::{Config, DevNetwork, DevSetup, Server};

/// How long a test waits for the server to read from its socket.
const READY_TIMEOUT: Duration = Duration::from_secs(20);

/// A network on free ports, so a test and a running `chat-dev` keep out of each other's way.
fn ephemeral() -> Config {
    Config {
        control_port: 0,
        bind_ip: "127.0.0.1".parse().expect("an address"),
        emulator_ip: "10.0.2.2".parse().expect("an address"),
        server_port: 0,
        data_dir: None,
        no_server: false,
    }
}

/// The emulator's AS, published at an address this machine can reach.
///
/// `10.0.2.2` exists only inside an emulator, so a test that uses the emulator's description needs
/// a different address. The AS, its SNAP endpoint and its path to the server stay the same.
fn emulator_on_loopback() -> Config {
    Config {
        emulator_ip: "127.0.0.1".parse().expect("an address"),
        ..ephemeral()
    }
}

/// Reads the description over plain TCP, as a client reads it.
async fn describe(control_url: &str) -> DevNetwork {
    reqwest::get(format!("{control_url}/info"))
        .await
        .expect("the control api answers")
        .json()
        .await
        .expect("a description")
}

/// Builds the client that a description describes.
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

/// Waits for the server. The socket binds before the endpoint reads, so early calls can fail.
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

    // Read it back over TCP, as a client that started no part of this process reads it.
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

/// Give each client its own token. See [`DevNetwork::auth_token`].
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
    // Each read gives the same answer for every field except the token.
    assert_eq!(first.endhost_api_url, second.endhost_api_url);
    assert_eq!(first.target, second.target);
    assert_eq!(first.ca_fingerprint, second.ca_fingerprint);

    stop.cancel();
    serving.await.expect("serving should not panic");
}

/// Two people in one room. One client shows the transport works, and two show that both tunnels
/// stay up.
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

    // A read as the second client shows the message crossed the link. A read as the first client
    // shows its tunnel still carries traffic.
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

/// A client in the emulator's AS and a client in the default AS share one room at the same time.
#[tokio::test(flavor = "multi_thread")]
async fn a_client_in_the_emulator_as_shares_a_room_with_a_local_one() {
    let setup = DevSetup::start(&emulator_on_loopback())
        .await
        .expect("a network");
    let default = setup.network().clone();
    let emulator = setup.emulator_network().clone();
    let stop = setup.stopper();
    let serving = tokio::spawn(setup.serve());

    assert_ne!(
        default.client_isd_as, emulator.client_isd_as,
        "the two clients attach to different ASes",
    );
    assert_ne!(
        default.endhost_api_url, emulator.endhost_api_url,
        "each AS has an endhost API of its own",
    );

    let here = client(&default).await;
    let there = client(&emulator).await;
    await_ready(&here).await;

    for (client, who) in [(&here, "ada"), (&there, "grace")] {
        client
            .register(who, "a password")
            .await
            .expect("an account");
        client.login(who, "a password").await.expect("a session");
    }

    let room = here.create_room("general").await.expect("a room");
    here.send(room.id, "from this machine").await.expect("sent");
    there
        .send(room.id, "from the emulator")
        .await
        .expect("sent");

    let expected = [
        ("ada".to_owned(), "from this machine".to_owned()),
        ("grace".to_owned(), "from the emulator".to_owned()),
    ];
    for (client, who) in [(&there, "the emulator"), (&here, "this machine")] {
        let seen: Vec<(String, String)> = client
            .messages_newest(room.id, 10)
            .await
            .expect("the room reads back")
            .into_iter()
            .map(|message| (message.username, message.body))
            .collect();

        assert_eq!(seen, expected, "as read from {who}");
    }

    stop.cancel();
    serving.await.expect("serving should not panic");
}

/// Each client gets its own address, and the other client gets a different one.
#[tokio::test(flavor = "multi_thread")]
async fn only_the_emulator_is_told_the_emulator_address() {
    let setup = DevSetup::start(&ephemeral()).await.expect("a network");
    let control_url = setup.network().control_url.clone();
    let port = control_url
        .rsplit_once(':')
        .map(|(_, port)| port.to_owned())
        .expect("a port");
    let stop = setup.stopper();
    let serving = tokio::spawn(setup.serve());

    // A terminal client asks. The Host header carries the address it wrote in its URL.
    let default = describe(&control_url).await;
    // The emulator asks. It reaches the same socket, and it wrote a different URL.
    let emulator: DevNetwork = reqwest::Client::new()
        .get(format!("{control_url}/info"))
        .header("host", format!("10.0.2.2:{port}"))
        .send()
        .await
        .expect("the control api answers")
        .json()
        .await
        .expect("a description");

    assert!(
        default.endhost_api_url.contains("127.0.0.1"),
        "a client at the bound address keeps loopback: {}",
        default.endhost_api_url,
    );
    assert!(
        emulator.endhost_api_url.contains("10.0.2.2"),
        "the emulator is told the address it can reach: {}",
        emulator.endhost_api_url,
    );

    // One network, two ways in. Each field that names the server holds the same value.
    assert_eq!(default.target, emulator.target);
    assert_eq!(default.base_url, emulator.base_url);
    assert_eq!(default.ca_fingerprint, emulator.ca_fingerprint);
    assert_eq!(
        default.server_endhost_api_url,
        emulator.server_endhost_api_url
    );

    stop.cancel();
    serving.await.expect("serving should not panic");
}

/// Refused before the topology starts. The failure underneath arrives much later, and it says
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
}

/// The network without the chat server, for a reader who starts the server.
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
    // This process makes the certificate, so the description holds it under `--no-server` too.
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
