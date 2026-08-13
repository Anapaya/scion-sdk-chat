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
//! Handler tests. They drive the real router against a real store in a temporary directory, so
//! nothing here is a stand-in except the network.

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode, header},
};
use clap::Parser as _;
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt as _;

use super::router;
use crate::config::Config;

/// A router over a fresh database, with the directory kept alive for the test's duration.
///
/// Extra `args` override the defaults, which is how the cap tests get a cap they can reach.
async fn app(args: &[&str]) -> (Router, TempDir) {
    let dir = TempDir::new().expect("temp dir");
    let mut argv = vec![
        "chat-server",
        "--transport",
        "tcp",
        "--data-dir",
        dir.path().to_str().expect("utf-8 path"),
    ];
    argv.extend_from_slice(args);

    let state = crate::state(&Config::parse_from(argv))
        .await
        .expect("state");
    (router(state), dir)
}

/// Sends a request and returns the status with the body decoded as JSON.
///
/// A body of `Value::Null` stands for "no body", which is what the empty responses produce.
async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");

    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, body)
}

fn get(path: &str) -> Request<Body> {
    Request::builder()
        .uri(path)
        .body(Body::empty())
        .expect("request")
}

fn post(path: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("request")
}

/// Adds the bearer token to a request.
fn bearer(mut request: Request<Body>, token: &str) -> Request<Body> {
    request.headers_mut().insert(
        header::AUTHORIZATION,
        format!("Bearer {token}").parse().expect("header"),
    );
    request
}

/// Registers an account and logs in, returning the token.
async fn login(app: &Router, username: &str) -> String {
    let (status, _) = send(
        app,
        post(
            "/api/v1/register",
            json!({ "username": username, "password": "correct horse battery staple" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, body) = send(
        app,
        post(
            "/api/v1/login",
            json!({ "username": username, "password": "correct horse battery staple" }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    body["token"].as_str().expect("a token").to_owned()
}

/// The error code a failing response carries, which is what a client branches on.
fn code(body: &Value) -> &str {
    body["error"]["code"].as_str().unwrap_or("<no code>")
}

#[tokio::test]
async fn healthz_answers_without_a_token() {
    let (app, _dir) = app(&[]).await;

    let (status, body) = send(&app, get("/api/v1/healthz")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn server_info_reports_the_configured_limits_without_a_token() {
    let (app, _dir) = app(&["--max-accounts", "7", "--token-expiry-days", "2"]).await;

    let (status, body) = send(&app, get("/api/v1/server")).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["max_accounts"], 7);
    assert_eq!(body["token_validity_seconds"], 2 * 86_400);
    assert_eq!(body["isd_as"], Value::Null, "there is no SCION address yet");
}

#[tokio::test]
async fn every_other_endpoint_refuses_a_request_without_a_token() {
    let (app, _dir) = app(&[]).await;

    for request in [
        get("/api/v1/rooms"),
        post("/api/v1/rooms", json!({ "name": "scion" })),
        get("/api/v1/rooms/1/messages"),
        post("/api/v1/rooms/1/messages", json!({ "body": "hi" })),
    ] {
        let uri = request.uri().to_string();
        let (status, body) = send(&app, request).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
        assert_eq!(code(&body), "unauthorized", "{uri}");
    }
}

#[tokio::test]
async fn a_token_that_was_not_signed_by_this_server_is_refused() {
    let (app, _dir) = app(&[]).await;

    let (status, body) = send(&app, bearer(get("/api/v1/rooms"), "not.a.token")).await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(code(&body), "unauthorized");
}

#[tokio::test]
async fn registering_twice_reports_the_name_as_taken() {
    let (app, _dir) = app(&[]).await;
    let body = json!({ "username": "alice", "password": "correct horse battery staple" });

    let (first, _) = send(&app, post("/api/v1/register", body.clone())).await;
    let (second, error) = send(&app, post("/api/v1/register", body)).await;

    assert_eq!(first, StatusCode::CREATED);
    assert_eq!(second, StatusCode::CONFLICT);
    assert_eq!(code(&error), "username_taken");
}

#[tokio::test]
async fn a_username_outside_the_accepted_shape_is_refused() {
    let (app, _dir) = app(&[]).await;

    for username in ["", &"a".repeat(33), "with\ncontrol"] {
        let (status, body) = send(
            &app,
            post(
                "/api/v1/register",
                json!({ "username": username, "password": "x" }),
            ),
        )
        .await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{username:?}");
        assert_eq!(code(&body), "invalid_username", "{username:?}");
    }
}

#[tokio::test]
async fn the_account_cap_is_reported_as_a_refusal_to_take_more() {
    let (app, _dir) = app(&["--max-accounts", "1"]).await;
    login(&app, "alice").await;

    let (status, body) = send(
        &app,
        post(
            "/api/v1/register",
            json!({ "username": "bob", "password": "x" }),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(code(&body), "cap_exceeded");
}

#[tokio::test]
async fn a_wrong_password_and_an_unknown_account_are_reported_alike() {
    let (app, _dir) = app(&[]).await;
    login(&app, "alice").await;

    let (wrong, wrong_body) = send(
        &app,
        post(
            "/api/v1/login",
            json!({ "username": "alice", "password": "nope" }),
        ),
    )
    .await;
    let (unknown, unknown_body) = send(
        &app,
        post(
            "/api/v1/login",
            json!({ "username": "nobody", "password": "nope" }),
        ),
    )
    .await;

    assert_eq!(wrong, StatusCode::UNAUTHORIZED);
    assert_eq!(unknown, StatusCode::UNAUTHORIZED);
    assert_eq!(
        wrong_body, unknown_body,
        "the two must be indistinguishable, or the API reveals which names are registered"
    );
}

#[tokio::test]
async fn a_malformed_body_is_reported_in_the_same_envelope() {
    let (app, _dir) = app(&[]).await;
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/register")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{not json"))
        .expect("request");

    let (status, body) = send(&app, request).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(code(&body), "invalid_body");
}

#[tokio::test]
async fn the_lobby_is_listed_from_the_first_request() {
    let (app, _dir) = app(&[]).await;
    let token = login(&app, "alice").await;

    let (status, body) = send(&app, bearer(get("/api/v1/rooms"), &token)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["rooms"][0]["name"], "lobby");
    assert_eq!(body["rooms"][0]["latest_seq"], 0);
}

#[tokio::test]
async fn creating_a_room_reports_where_it_is_and_creating_it_again_does_not() {
    let (app, _dir) = app(&[]).await;
    let token = login(&app, "alice").await;

    let created = app
        .clone()
        .oneshot(bearer(
            post("/api/v1/rooms", json!({ "name": "scion" })),
            &token,
        ))
        .await
        .expect("response");
    assert_eq!(created.status(), StatusCode::CREATED);
    assert_eq!(
        created
            .headers()
            .get(header::LOCATION)
            .expect("a 201 says where the room is")
            .to_str()
            .expect("ascii"),
        "/api/v1/rooms/2"
    );

    // The same name, in a different case: nothing is created, so nothing is reported as created.
    let again = app
        .clone()
        .oneshot(bearer(
            post("/api/v1/rooms", json!({ "name": "SCION" })),
            &token,
        ))
        .await
        .expect("response");

    assert_eq!(again.status(), StatusCode::OK);
    assert!(again.headers().get(header::LOCATION).is_none());
}

#[tokio::test]
async fn a_room_name_outside_the_accepted_shape_is_refused() {
    let (app, _dir) = app(&[]).await;
    let token = login(&app, "alice").await;

    for name in ["", &"a".repeat(65), "café"] {
        let (status, body) = send(
            &app,
            bearer(post("/api/v1/rooms", json!({ "name": name })), &token),
        )
        .await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{name:?}");
        assert_eq!(code(&body), "invalid_name", "{name:?}");
    }
}

#[tokio::test]
async fn the_room_cap_is_reported_as_a_refusal_to_take_more() {
    // One slot, and the lobby already holds it.
    let (app, _dir) = app(&["--max-rooms", "1"]).await;
    let token = login(&app, "alice").await;

    let (status, body) = send(
        &app,
        bearer(post("/api/v1/rooms", json!({ "name": "scion" })), &token),
    )
    .await;

    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(code(&body), "cap_exceeded");
}

#[tokio::test]
async fn a_posted_message_comes_back_attributed_to_the_token_holder() {
    let (app, _dir) = app(&[]).await;
    let token = login(&app, "alice").await;

    let (status, posted) = send(
        &app,
        bearer(
            post("/api/v1/rooms/1/messages", json!({ "body": "hi" })),
            &token,
        ),
    )
    .await;
    let (_, page) = send(&app, bearer(get("/api/v1/rooms/1/messages"), &token)).await;

    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(posted["seq"], 1);
    assert_eq!(page["messages"][0]["username"], "alice");
    assert_eq!(page["messages"][0]["body"], "hi");
}

#[tokio::test]
async fn a_message_larger_than_the_limit_is_refused() {
    let (app, _dir) = app(&["--max-message-bytes", "8"]).await;
    let token = login(&app, "alice").await;

    let (status, body) = send(
        &app,
        bearer(
            post(
                "/api/v1/rooms/1/messages",
                json!({ "body": "far too long to fit" }),
            ),
            &token,
        ),
    )
    .await;

    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(code(&body), "message_too_large");
}

#[tokio::test]
async fn a_room_that_does_not_exist_is_reported_on_both_message_routes() {
    let (app, _dir) = app(&[]).await;
    let token = login(&app, "alice").await;

    let (reading, read_body) = send(&app, bearer(get("/api/v1/rooms/999/messages"), &token)).await;
    let (writing, write_body) = send(
        &app,
        bearer(
            post("/api/v1/rooms/999/messages", json!({ "body": "hi" })),
            &token,
        ),
    )
    .await;

    assert_eq!(reading, StatusCode::NOT_FOUND);
    assert_eq!(code(&read_body), "room_not_found");
    assert_eq!(writing, StatusCode::NOT_FOUND);
    assert_eq!(code(&write_body), "room_not_found");
}

#[tokio::test]
async fn the_three_fetch_shapes_return_what_the_contract_says() {
    let (app, _dir) = app(&[]).await;
    let token = login(&app, "alice").await;
    for body in ["one", "two", "three"] {
        send(
            &app,
            bearer(
                post("/api/v1/rooms/1/messages", json!({ "body": body })),
                &token,
            ),
        )
        .await;
    }

    let seqs = |page: &Value| {
        page["messages"]
            .as_array()
            .expect("an array")
            .iter()
            .map(|m| m["seq"].as_u64().expect("a seq"))
            .collect::<Vec<_>>()
    };

    let (_, newest) = send(&app, bearer(get("/api/v1/rooms/1/messages"), &token)).await;
    let (_, after) = send(
        &app,
        bearer(get("/api/v1/rooms/1/messages?after_seq=1"), &token),
    )
    .await;
    let (_, before) = send(
        &app,
        bearer(get("/api/v1/rooms/1/messages?before_seq=3"), &token),
    )
    .await;
    let (_, limited) = send(
        &app,
        bearer(get("/api/v1/rooms/1/messages?limit=2"), &token),
    )
    .await;

    assert_eq!(seqs(&newest), [1, 2, 3]);
    assert_eq!(seqs(&after), [2, 3]);
    assert_eq!(seqs(&before), [1, 2]);
    assert_eq!(
        seqs(&limited),
        [2, 3],
        "a limit takes the newest, not the oldest"
    );
}

#[tokio::test]
async fn the_two_cursors_cannot_be_combined() {
    let (app, _dir) = app(&[]).await;
    let token = login(&app, "alice").await;

    let (status, body) = send(
        &app,
        bearer(
            get("/api/v1/rooms/1/messages?after_seq=1&before_seq=3"),
            &token,
        ),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(code(&body), "invalid_body");
}
