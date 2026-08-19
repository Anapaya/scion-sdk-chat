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
//! [`TcpTransport`] against the real server, embedded as a library.
//!
//! The real server rather than a hand-written stand-in, because a stand-in can drift from the API
//! and this cannot.

use std::time::Duration;

use bytes::Bytes;
use chat_client_core::{
    ChatClient, ChatError, ClientConfig, PollConfig, RoomEvent, Since, TcpTransport,
    Transport as _, TransportKind,
    v1::{RoomId, Seq},
};
use clap::Parser as _;
use tempfile::TempDir;

/// Starts the server on a port the operating system picks, and returns its base URL.
///
/// The listener is bound here rather than by the server, which is the only way to learn the port it
/// was given. The directory is returned so the test holds it open for as long as the server runs.
async fn server() -> (String, TempDir) {
    let dir = TempDir::new().expect("temp dir");
    let config = chat_server::config::Config::parse_from([
        "chat-server",
        "--transport",
        "tcp",
        "--data-dir",
        dir.path().to_str().expect("utf-8 path"),
    ]);

    let state = chat_server::state(&config).await.expect("state");
    let router = chat_server::api::router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("a free port");
    let address = listener.local_addr().expect("the bound address");
    tokio::spawn(async move {
        axum::serve(listener, router).await.expect("serving");
    });

    (format!("http://{address}"), dir)
}

/// The configuration a client uses against `base`.
fn config(base: &str) -> ClientConfig {
    ClientConfig {
        transport: TransportKind::Tcp,
        server_url: base.parse().expect("a url"),
        ..ClientConfig::default()
    }
}

fn transport() -> TcpTransport {
    TcpTransport::new().expect("a transport")
}

/// The acceptance criterion: a request built by hand, with no client above it, reaches the real
/// server and comes back.
#[tokio::test]
async fn a_hand_built_request_reaches_the_server_and_the_reply_comes_back() {
    let (base, _dir) = server().await;
    let request = http::Request::get(format!("{base}/api/v1/healthz"))
        .body(Bytes::new())
        .expect("a request");

    let reply = transport().request(request).await.expect("a reply");

    assert_eq!(reply.status(), 200);
    assert_eq!(
        reply.headers()[http::header::CONTENT_TYPE],
        "application/json",
    );
    assert_eq!(reply.body(), &Bytes::from(r#"{"status":"ok"}"#));
}

/// A body and its content type survive the crossing, which is what every write endpoint needs.
#[tokio::test]
async fn a_posted_body_arrives_as_it_was_written() {
    let (base, _dir) = server().await;
    let request = http::Request::post(format!("{base}/api/v1/register"))
        .header(http::header::CONTENT_TYPE, "application/json")
        .body(Bytes::from(
            r#"{"username":"alice","password":"correct horse battery staple"}"#,
        ))
        .expect("a request");

    let reply = transport().request(request).await.expect("a reply");

    assert_eq!(reply.status(), 201, "the account was created");
}

/// A refusal is a reply, not a transport failure: reading the status is the layer above's job.
#[tokio::test]
async fn a_refusal_comes_back_as_a_reply_rather_than_an_error() {
    let (base, _dir) = server().await;
    let request = http::Request::get(format!("{base}/api/v1/rooms"))
        .body(Bytes::new())
        .expect("a request");

    let reply = transport().request(request).await.expect("a reply");

    assert_eq!(reply.status(), 401, "no bearer token was sent");
    assert!(
        reply.body().starts_with(br#"{"error":"#),
        "the server's error envelope reaches the caller untouched: {:?}",
        reply.body(),
    );
}

/// The whole client against the whole server: register, log in, list, create, post, read back, then
/// page backwards. One scenario rather than a test per call, because what it proves is that the
/// sequence works — the wire form of each call is asserted against the mock.
#[tokio::test]
async fn the_client_speaks_the_real_protocol_end_to_end() {
    let (base, _dir) = server().await;
    let client = ChatClient::new(config(&base)).await.expect("a client");

    client.health().await.expect("the server is up");
    let info = client.server_info().await.expect("the metadata");
    assert_eq!(info.isd_as, None, "this server is on tcp");

    client
        .register("alice", "correct horse battery staple")
        .await
        .expect("the account is created");
    let session = client
        .login("alice", "correct horse battery staple")
        .await
        .expect("a login");
    assert_eq!(session.username, "alice");
    assert_eq!(client.session(), Some(session));

    let rooms = client.rooms().await.expect("the rooms");
    assert_eq!(rooms.len(), 1, "only the lobby exists at first");
    assert_eq!(rooms[0].name, "lobby");

    let room = client.create_room("scion").await.expect("a new room");
    assert_eq!(room.name, "scion");
    assert_eq!(room.latest_seq, Seq::START, "nothing has been posted yet");

    let first = client.send(room.id, "hello").await.expect("a message");
    let second = client.send(room.id, "again").await.expect("a message");
    assert!(second.seq > first.seq, "seq only ever increases");

    let newest = client
        .messages_newest(room.id, 50)
        .await
        .expect("the newest page");
    assert_eq!(
        newest
            .iter()
            .map(|message| message.body.as_str())
            .collect::<Vec<_>>(),
        ["hello", "again"],
        "oldest first, whichever cursor asked",
    );
    assert_eq!(
        newest[0].username, "alice",
        "attributed to the token holder"
    );

    let after = client
        .messages_after(room.id, first.seq, 50)
        .await
        .expect("what came later");
    assert_eq!(
        after.iter().map(|message| message.seq).collect::<Vec<_>>(),
        [second.seq],
        "the cursor is exclusive",
    );

    let older = client
        .messages_before(room.id, second.seq, 50)
        .await
        .expect("what came earlier");
    assert_eq!(
        older.iter().map(|message| message.seq).collect::<Vec<_>>(),
        [first.seq],
        "paging backwards stops before the cursor",
    );
}

/// The server's refusals arrive as refusals, with the code the caller branches on.
#[tokio::test]
async fn the_servers_own_refusals_arrive_with_their_codes() {
    let (base, _dir) = server().await;
    let client = ChatClient::new(config(&base)).await.expect("a client");

    let error = client.rooms().await.expect_err("nobody is logged in");
    assert!(
        matches!(error, ChatError::NotLoggedIn),
        "the client stops this before the wire: {error:?}",
    );

    client
        .register("alice", "correct horse battery staple")
        .await
        .expect("the account is created");
    let error = client
        .register("alice", "correct horse battery staple")
        .await
        .expect_err("that name is taken");
    assert!(
        matches!(
            error,
            ChatError::Api {
                status: 409,
                code: chat_client_core::v1::ErrorCode::UsernameTaken,
                ..
            }
        ),
        "expected the name to be reported as taken, got {error:?}",
    );

    let error = client
        .login("alice", "the wrong password")
        .await
        .expect_err("that password is wrong");
    assert!(
        matches!(
            error,
            ChatError::Api {
                status: 401,
                code: chat_client_core::v1::ErrorCode::InvalidCredentials,
                ..
            }
        ),
        "a login carries no token, so its 401 is about the password: {error:?}",
    );

    client
        .login("alice", "correct horse battery staple")
        .await
        .expect("a login");
    let error = client
        .messages_newest(RoomId::new(999), 50)
        .await
        .expect_err("no such room");
    assert!(
        matches!(
            error,
            ChatError::Api {
                status: 404,
                code: chat_client_core::v1::ErrorCode::RoomNotFound,
                ..
            }
        ),
        "expected the room to be reported missing, got {error:?}",
    );
}

/// Logging out is enough to stop authenticated calls: the client refuses before the wire.
#[tokio::test]
async fn logging_out_stops_authenticated_calls() {
    let (base, _dir) = server().await;
    let client = ChatClient::new(config(&base)).await.expect("a client");

    client
        .register("alice", "correct horse battery staple")
        .await
        .expect("the account is created");
    client
        .login("alice", "correct horse battery staple")
        .await
        .expect("a login");
    client.rooms().await.expect("the rooms");

    client.logout();

    let error = client.rooms().await.expect_err("the login is gone");
    assert!(
        matches!(error, ChatError::NotLoggedIn),
        "expected to be told to log in, got {error:?}",
    );
}

/// The feed against the real server: open a room, post, and watch it arrive.
///
/// The interval is set to nothing so the test does not wait out a real two seconds. What the
/// cadence is under a controlled clock is asserted in the unit tests.
#[tokio::test]
async fn a_feed_delivers_what_is_posted_while_it_is_open() {
    let (base, _dir) = server().await;
    let client = ChatClient::new(ClientConfig {
        poll: PollConfig {
            room_interval: Duration::ZERO,
            page_limit: 50,
        },
        ..config(&base)
    })
    .await
    .expect("a client");

    client
        .register("alice", "correct horse battery staple")
        .await
        .expect("the account is created");
    client
        .login("alice", "correct horse battery staple")
        .await
        .expect("a login");
    let room = client.create_room("scion").await.expect("a room");
    let first = client
        .send(room.id, "before watching")
        .await
        .expect("a message");

    let mut feed = client
        .watch_room(room.id, Since::Newest { limit: 50 })
        .await
        .expect("a feed");

    let backfill = feed.next().await.expect("the backfill");
    assert_eq!(
        backfill,
        RoomEvent::Messages(
            client
                .messages_newest(room.id, 50)
                .await
                .expect("the same page"),
        ),
        "the opening batch is the newest page",
    );

    let second = client
        .send(room.id, "while watching")
        .await
        .expect("a message");
    let event = feed.next().await.expect("what came after");

    let RoomEvent::Messages(messages) = event else {
        panic!("expected a batch, got {event:?}");
    };
    assert_eq!(
        messages
            .iter()
            .map(|message| message.seq)
            .collect::<Vec<_>>(),
        [second.seq],
        "only what the cursor had not passed, and the cursor started at {first:?}",
    );
    assert_eq!(messages[0].body, "while watching");
}

/// A room that does not exist fails where the feed is opened.
#[tokio::test]
async fn watching_a_room_that_does_not_exist_fails_at_once() {
    let (base, _dir) = server().await;
    let client = ChatClient::new(config(&base)).await.expect("a client");
    client
        .register("alice", "correct horse battery staple")
        .await
        .expect("the account is created");
    client
        .login("alice", "correct horse battery staple")
        .await
        .expect("a login");

    let Err(error) = client
        .watch_room(RoomId::new(999), Since::Newest { limit: 50 })
        .await
    else {
        panic!("expected no such room");
    };

    assert!(
        matches!(
            error,
            ChatError::Api {
                code: chat_client_core::v1::ErrorCode::RoomNotFound,
                ..
            }
        ),
        "expected the room to be reported missing, got {error:?}",
    );
}
