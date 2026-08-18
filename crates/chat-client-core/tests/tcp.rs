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

use bytes::Bytes;
use chat_client_core::{ClientConfig, TcpTransport, Transport as _, TransportKind};
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

fn transport() -> TcpTransport {
    TcpTransport::new(&ClientConfig {
        transport: TransportKind::Tcp,
        ..ClientConfig::default()
    })
    .expect("a transport")
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
