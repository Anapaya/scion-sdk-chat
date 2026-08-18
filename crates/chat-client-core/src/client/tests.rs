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
//! Tests for the request each method sends and the reply it decodes.
//!
//! The wire form is asserted against a mock that records requests, so these are the cheapest guard
//! against drifting from the server's contract.

use chat_core::api::v1::ErrorCode;

use super::*;
use crate::{error::TransportError, transport::mock::MockTransport};

/// A token whose contents never matter: the client carries it, it does not read it.
const TOKEN: &str = "a.b.c";

/// The reply `login` needs before any authenticated call will go out.
const LOGIN_JSON: &str = r#"{"token":"a.b.c","expires_at":1893456000000}"#;

/// A client over `mock`, with nothing logged in.
fn client(mock: &MockTransport) -> ChatClient {
    ChatClient::new_with_transport(
        Arc::new(mock.clone()),
        Url::parse("http://host:8080").expect("a url"),
        PollConfig::default(),
    )
}

/// A client over `mock` that has logged in, for the calls that need a token.
async fn logged_in(mock: &MockTransport) -> ChatClient {
    let client = client(mock);
    client.login("alice", "a password").await.expect("a login");

    client
}

/// A mock that answers a login and one other route.
fn scripted(route: &str, status: u16, body: &'static str) -> MockTransport {
    MockTransport::new()
        .respond("POST /api/v1/login", 200, LOGIN_JSON)
        .respond(route, status, body)
}

/// The bearer header a request should be carrying, once someone has logged in.
fn sent_token(request: &http::Request<Bytes>) -> &str {
    request.headers()[AUTHORIZATION]
        .to_str()
        .expect("an ascii header")
}

#[tokio::test]
async fn register_posts_the_credentials_and_needs_no_token() {
    let mock = MockTransport::new().respond("POST /api/v1/register", 201, "");

    client(&mock)
        .register("alice", "a password")
        .await
        .expect("the account is created");

    let request = mock.last_request();
    assert_eq!(request.method(), Method::POST);
    assert_eq!(request.uri().path(), "/api/v1/register");
    assert_eq!(request.headers()[CONTENT_TYPE], "application/json");
    assert!(!request.headers().contains_key(AUTHORIZATION));
    assert_eq!(
        request.body(),
        &Bytes::from(r#"{"username":"alice","password":"a password"}"#),
    );
}

#[tokio::test]
async fn login_posts_the_credentials_and_keeps_the_token_to_itself() {
    let mock = MockTransport::new().respond("POST /api/v1/login", 200, LOGIN_JSON);

    let session = client(&mock)
        .login("alice", "a password")
        .await
        .expect("a login");

    let request = mock.last_request();
    assert_eq!(request.method(), Method::POST);
    assert_eq!(request.uri().path(), "/api/v1/login");
    assert_eq!(
        request.body(),
        &Bytes::from(r#"{"username":"alice","password":"a password"}"#),
    );
    assert_eq!(session.username, "alice");
    assert_eq!(session.expires_at, UnixMillis::new(1_893_456_000_000));
}

#[tokio::test]
async fn health_gets_the_liveness_route_without_a_token() {
    let mock = MockTransport::new().respond("GET /api/v1/healthz", 200, r#"{"status":"ok"}"#);

    client(&mock).health().await.expect("the server is up");

    let request = mock.last_request();
    assert_eq!(request.method(), Method::GET);
    assert_eq!(request.uri().path(), "/api/v1/healthz");
    assert!(!request.headers().contains_key(AUTHORIZATION));
}

#[tokio::test]
async fn server_info_gets_the_metadata_route_without_a_token() {
    let body = r#"{"version":"0.1.0","isd_as":null,"max_accounts":500,"max_rooms":100,
                   "max_message_bytes":4096,"token_validity_seconds":604800}"#;
    let mock = MockTransport::new().respond("GET /api/v1/server", 200, body);

    let info = client(&mock).server_info().await.expect("the metadata");

    assert_eq!(mock.last_request().uri().path(), "/api/v1/server");
    assert_eq!(info.max_rooms, 100);
    assert_eq!(info.isd_as, None);
}

#[tokio::test]
async fn rooms_gets_the_rooms_route_and_unwraps_the_envelope() {
    let body = r#"{"rooms":[{"id":1,"name":"lobby","latest_seq":0}]}"#;
    let mock = scripted("GET /api/v1/rooms", 200, body);

    let rooms = logged_in(&mock).await.rooms().await.expect("the rooms");

    let request = mock.last_request();
    assert_eq!(request.method(), Method::GET);
    assert_eq!(request.uri().path(), "/api/v1/rooms");
    assert_eq!(sent_token(&request), format!("Bearer {TOKEN}"));
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0].name, "lobby");
}

#[tokio::test]
async fn create_room_posts_the_name() {
    let body = r#"{"id":2,"name":"scion","latest_seq":0}"#;
    let mock = scripted("POST /api/v1/rooms", 201, body);

    let room = logged_in(&mock)
        .await
        .create_room("scion")
        .await
        .expect("the room");

    let request = mock.last_request();
    assert_eq!(request.method(), Method::POST);
    assert_eq!(request.uri().path(), "/api/v1/rooms");
    assert_eq!(request.headers()[CONTENT_TYPE], "application/json");
    assert_eq!(request.body(), &Bytes::from(r#"{"name":"scion"}"#));
    assert_eq!(room.id, RoomId::new(2));
}

#[tokio::test]
async fn send_posts_the_body_to_the_room() {
    let body = r#"{"seq":7,"posted_at":1893456000000}"#;
    let mock = scripted("POST /api/v1/rooms/1/messages", 201, body);

    let posted = logged_in(&mock)
        .await
        .send(RoomId::new(1), "hi")
        .await
        .expect("the message lands");

    let request = mock.last_request();
    assert_eq!(request.method(), Method::POST);
    assert_eq!(request.uri().path(), "/api/v1/rooms/1/messages");
    assert_eq!(request.headers()[CONTENT_TYPE], "application/json");
    assert_eq!(sent_token(&request), format!("Bearer {TOKEN}"));
    assert_eq!(request.body(), &Bytes::from(r#"{"body":"hi"}"#));
    assert_eq!(posted.seq, Seq::new(7));
}

#[tokio::test]
async fn messages_newest_asks_for_a_page_with_no_cursor() {
    let mock = scripted("GET /api/v1/rooms/1/messages", 200, r#"{"messages":[]}"#);

    logged_in(&mock)
        .await
        .messages_newest(RoomId::new(1), 50)
        .await
        .expect("a page");

    let request = mock.last_request();
    assert_eq!(request.uri().path(), "/api/v1/rooms/1/messages");
    assert_eq!(request.uri().query(), Some("limit=50"));
}

#[tokio::test]
async fn messages_after_asks_forwards_from_the_cursor() {
    let body = r#"{"messages":[{"seq":8,"username":"bob","body":"hi","posted_at":1893456000000}]}"#;
    let mock = scripted("GET /api/v1/rooms/1/messages", 200, body);

    let messages = logged_in(&mock)
        .await
        .messages_after(RoomId::new(1), Seq::new(7), 50)
        .await
        .expect("a page");

    let request = mock.last_request();
    assert_eq!(request.uri().path(), "/api/v1/rooms/1/messages");
    assert_eq!(request.uri().query(), Some("limit=50&after_seq=7"));
    assert_eq!(messages[0].seq, Seq::new(8));
}

#[tokio::test]
async fn messages_before_asks_backwards_from_the_cursor() {
    let mock = scripted("GET /api/v1/rooms/1/messages", 200, r#"{"messages":[]}"#);

    logged_in(&mock)
        .await
        .messages_before(RoomId::new(1), Seq::new(7), 20)
        .await
        .expect("a page");

    let request = mock.last_request();
    assert_eq!(request.uri().path(), "/api/v1/rooms/1/messages");
    assert_eq!(request.uri().query(), Some("limit=20&before_seq=7"));
}

/// The base and the path are joined in one place, so a trailing slash cannot double up.
#[tokio::test]
async fn a_server_url_with_a_trailing_slash_does_not_double_the_separator() {
    let mock = MockTransport::new().respond("GET /api/v1/healthz", 200, "{}");
    let client = ChatClient::new_with_transport(
        Arc::new(mock.clone()),
        Url::parse("http://host:8080/").expect("a url"),
        PollConfig::default(),
    );

    client.health().await.expect("the server is up");

    assert_eq!(mock.last_request().uri(), "http://host:8080/api/v1/healthz",);
}

#[tokio::test]
async fn an_authenticated_call_without_a_login_never_reaches_the_wire() {
    let mock = MockTransport::new();

    let error = client(&mock)
        .rooms()
        .await
        .expect_err("nobody is logged in");

    assert!(
        matches!(error, ChatError::NotLoggedIn),
        "expected to be told to log in, got {error:?}",
    );
    assert_eq!(mock.request_count(), 0, "nothing was sent");
}

/// A refused token ends the session, and the client stops claiming a login it cannot use.
#[tokio::test]
async fn a_refused_token_ends_the_session_and_is_forgotten() {
    let expired = r#"{"error":{"code":"expired_token","message":"log in again"}}"#;
    let mock = scripted("GET /api/v1/rooms", 401, expired);
    let client = logged_in(&mock).await;
    assert!(client.session().is_some(), "logged in to begin with");

    let error = client.rooms().await.expect_err("the token is refused");

    assert!(
        matches!(error, ChatError::SessionExpired),
        "expected the session to end, got {error:?}",
    );
    assert_eq!(client.session(), None, "the token is not worth keeping");
}

/// Having forgotten the token, the next call says so instead of spending a request to be refused
/// again.
#[tokio::test]
async fn a_call_after_a_refused_token_never_reaches_the_wire() {
    let expired = r#"{"error":{"code":"expired_token","message":"log in again"}}"#;
    let mock = scripted("GET /api/v1/rooms", 401, expired);
    let client = logged_in(&mock).await;
    client.rooms().await.expect_err("the token is refused");
    let sent = mock.request_count();

    let error = client.rooms().await.expect_err("nothing is logged in now");

    assert!(
        matches!(error, ChatError::NotLoggedIn),
        "expected to be told to log in, got {error:?}",
    );
    assert_eq!(mock.request_count(), sent, "no second request went out");
}

/// A 401 on a call that carried no token is not a session ending — there was no session. A wrong
/// password arrives as the code the server sent.
#[tokio::test]
async fn a_401_without_a_token_reports_what_the_server_said() {
    let refused = r#"{"error":{"code":"invalid_credentials","message":"no match"}}"#;
    let mock = MockTransport::new().respond("POST /api/v1/login", 401, refused);

    let error = client(&mock)
        .login("alice", "the wrong password")
        .await
        .expect_err("that password is wrong");

    let ChatError::Api { status, code, .. } = error else {
        panic!("expected an api error, got {error:?}");
    };
    assert_eq!(status, 401);
    assert_eq!(code, ErrorCode::InvalidCredentials);
}

#[tokio::test]
async fn a_refusal_the_server_explained_arrives_with_its_code() {
    let refused = r#"{"error":{"code":"room_not_found","message":"no room with that id"}}"#;
    let mock = scripted("GET /api/v1/rooms/9/messages", 404, refused);

    let error = logged_in(&mock)
        .await
        .messages_newest(RoomId::new(9), 50)
        .await
        .expect_err("no such room");

    let ChatError::Api { status, code, .. } = error else {
        panic!("expected an api error, got {error:?}");
    };
    assert_eq!(status, 404);
    assert_eq!(code, ErrorCode::RoomNotFound);
}

/// A reply the client cannot read is the server's fault, not the caller's, and it says so instead
/// of pretending the call succeeded.
#[tokio::test]
async fn a_reply_the_client_cannot_read_is_a_protocol_failure() {
    let mock = scripted("GET /api/v1/rooms", 200, "{}");

    let error = logged_in(&mock)
        .await
        .rooms()
        .await
        .expect_err("no rooms field");

    assert!(
        matches!(error, ChatError::Protocol(_)),
        "expected a protocol failure, got {error:?}",
    );
}

#[tokio::test]
async fn a_session_starts_absent_appears_on_login_and_goes_on_logout() {
    let mock = MockTransport::new().respond("POST /api/v1/login", 200, LOGIN_JSON);
    let client = client(&mock);

    assert_eq!(client.session(), None);
    client.login("alice", "a password").await.expect("a login");
    assert_eq!(
        client.session().map(|session| session.username),
        Some("alice".to_owned()),
    );

    client.logout();

    assert_eq!(client.session(), None);
}

/// Every clone is the same client, which is what lets a user interface hand copies around.
#[tokio::test]
async fn a_clone_shares_the_session() {
    let mock = MockTransport::new()
        .respond("POST /api/v1/login", 200, LOGIN_JSON)
        .respond("GET /api/v1/rooms", 200, r#"{"rooms":[]}"#);
    let client = client(&mock);
    let clone = client.clone();

    client.login("alice", "a password").await.expect("a login");

    clone.rooms().await.expect("the clone is logged in too");
    assert_eq!(sent_token(&mock.last_request()), format!("Bearer {TOKEN}"));

    clone.logout();
    assert_eq!(client.session(), None, "logging out is shared as well");
}

/// The transport's own failures pass through untouched: no retry, no rewording.
#[tokio::test]
async fn a_transport_failure_reaches_the_caller_as_it_was() {
    let mock = MockTransport::new()
        .respond("POST /api/v1/login", 200, LOGIN_JSON)
        .fail("GET /api/v1/rooms", TransportError::Timeout);

    let error = logged_in(&mock)
        .await
        .rooms()
        .await
        .expect_err("the request never completed");

    assert!(
        matches!(error, ChatError::Transport(TransportError::Timeout)),
        "expected the timeout to pass through, got {error:?}",
    );
}
