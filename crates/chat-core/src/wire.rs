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
//! The wire contract: every JSON body the chat API accepts or returns, and the error envelope
//! it fails with.
//!
//! The types are plain data — no logic, no validation, no defaults. Each carries the JSON it
//! serializes to, so a client author on a platform without these structs (the Kotlin and Swift
//! apps) can implement against the doc comments alone. Every example is also a test case in this
//! module, which is what keeps the two from drifting.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Identifier of a room, assigned by the server when the room is created. Stable for the room's
/// lifetime, and rooms are never deleted.
pub type RoomId = i64;

/// Position of a message in the server's message sequence.
///
/// `seq` increases strictly and never regresses, so it is the one total order that every client
/// agrees on. It is assigned server-wide rather than per room, so a single room's messages carry
/// gaps: treat `seq` as a cursor, never as a count. Clients remember the highest `seq` they have
/// seen in a room and poll for what came after it.
pub type Seq = i64;

/// A point in time on the wire: milliseconds since the Unix epoch, UTC.
pub type UnixMillis = i64;

/// Stands in for a secret in `Debug` output, so that no password or token can reach a log by way
/// of a struct that merely happens to be printed.
const REDACTED: &str = "<redacted>";

/// Request body of `POST /api/v1/register`.
///
/// The username is the account's permanent identity — the name messages are attributed to. There
/// is no rename, no password change, and no password reset.
///
/// ```json
/// {
///   "username": "alice",
///   "password": "correct horse battery staple"
/// }
/// ```
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegisterRequest {
    /// The name to register. UTF-8, 1–32 characters, no control characters; compared
    /// case-insensitively against the names already taken.
    pub username: String,
    /// The password in the clear — the connection's TLS is what protects it. The server keeps
    /// only a KDF hash.
    pub password: String,
}

impl fmt::Debug for RegisterRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisterRequest")
            .field("username", &self.username)
            .field("password", &REDACTED)
            .finish()
    }
}

/// Request body of `POST /api/v1/login`.
///
/// ```json
/// {
///   "username": "alice",
///   "password": "correct horse battery staple"
/// }
/// ```
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginRequest {
    /// The registered name to log in as.
    pub username: String,
    /// The password in the clear, verified against the stored hash.
    pub password: String,
}

impl fmt::Debug for LoginRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginRequest")
            .field("username", &self.username)
            .field("password", &REDACTED)
            .finish()
    }
}

/// Response body of `POST /api/v1/login`: the bearer token every other endpoint requires.
///
/// ```json
/// {
///   "token": "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhbGljZSIsImV4cCI6MTc5MDAwMDAwMH0.c2ln",
///   "expires_at": 1790000000000
/// }
/// ```
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginResponse {
    /// The JWT to send as `Authorization: Bearer <token>`. Opaque to clients: they carry it, they
    /// do not parse it.
    pub token: String,
    /// When the token stops being accepted. There are no refresh tokens — a client logs in again.
    pub expires_at: UnixMillis,
}

impl fmt::Debug for LoginResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginResponse")
            .field("token", &REDACTED)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// A room, as returned by `GET /api/v1/rooms` and `POST /api/v1/rooms`.
///
/// Every user belongs to every room: there is no membership, no joining, and no private rooms.
/// A room named `lobby` always exists.
///
/// ```json
/// {
///   "id": 1,
///   "name": "lobby",
///   "latest_seq": 42
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Room {
    /// The room's identifier, used in the message paths.
    pub id: RoomId,
    /// The room's display name. UTF-8, 1–64 characters, no control characters; unique
    /// case-insensitively.
    pub name: String,
    /// The `seq` of the newest message in the room, or `null` when the room has none yet.
    ///
    /// Comparing it against the newest message a client has read is what drives an unread badge,
    /// so one call to `GET /api/v1/rooms` covers every room at once. It is not a message count:
    /// `seq` is server-wide (see [`Seq`]).
    pub latest_seq: Option<Seq>,
}

/// Response body of `GET /api/v1/rooms`: every room on the server.
///
/// ```json
/// {
///   "rooms": [
///     { "id": 1, "name": "lobby", "latest_seq": 42 },
///     { "id": 2, "name": "scion", "latest_seq": null }
///   ]
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomsResponse {
    /// All rooms. Never empty: `lobby` is always present.
    pub rooms: Vec<Room>,
}

/// Request body of `POST /api/v1/rooms`.
///
/// Creation is idempotent on the name: an existing name returns that room instead of failing.
///
/// ```json
/// {
///   "name": "scion"
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateRoomRequest {
    /// The name to create. UTF-8, 1–64 characters, no control characters; matched
    /// case-insensitively against the rooms that already exist.
    pub name: String,
}

/// Request body of `POST /api/v1/rooms/{id}/messages`.
///
/// ```json
/// {
///   "body": "hello from 1-ff00:0:110"
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostMessageRequest {
    /// The message text. UTF-8, at most the server's `max_message_bytes` when encoded — see
    /// [`ServerInfo`].
    pub body: String,
}

/// Response body of `POST /api/v1/rooms/{id}/messages`: where the message landed.
///
/// A client does *not* advance its poll cursor to this `seq` — a message with a lower `seq` may
/// still be waiting to be polled. The cursor only ever follows what polling actually delivered.
///
/// ```json
/// {
///   "seq": 43,
///   "posted_at": 1789994400000
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostMessageResponse {
    /// The `seq` the server assigned to the new message.
    pub seq: Seq,
    /// When the server accepted the message. Server time — a client's own clock never appears on
    /// the wire.
    pub posted_at: UnixMillis,
}

/// One message in a room.
///
/// Messages are append-only: never edited, never deleted.
///
/// ```json
/// {
///   "seq": 43,
///   "username": "alice",
///   "body": "hello from 1-ff00:0:110",
///   "posted_at": 1789994400000
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// The message's position in the sequence, and the cursor used to fetch around it.
    pub seq: Seq,
    /// The account that posted it.
    pub username: String,
    /// The message text.
    pub body: String,
    /// When the server accepted the message.
    pub posted_at: UnixMillis,
}

/// Response body of `GET /api/v1/rooms/{id}/messages`, for all three ways of asking:
/// no cursor (the newest page, to open a room), `after_seq` (poll, then append), and
/// `before_seq` (load more, then prepend).
///
/// ```json
/// {
///   "messages": [
///     { "seq": 42, "username": "bob", "body": "anyone here?", "posted_at": 1789994300000 },
///     { "seq": 43, "username": "alice", "body": "hello", "posted_at": 1789994400000 }
///   ]
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessagesResponse {
    /// The page, **always oldest-first**, whichever cursor asked for it.
    ///
    /// A page shorter than the requested `limit` means the client has reached the end it was
    /// walking towards: the present when polling forwards, the start of history when loading
    /// older messages. A full page means more is waiting — ask again immediately.
    pub messages: Vec<Message>,
}

/// Response body of `GET /api/v1/server`: everything about the server a client may want to show
/// or adapt to. Unauthenticated, so a client can read it before anyone logs in.
///
/// ```json
/// {
///   "version": "0.1.0",
///   "isd_as": "1-ff00:0:110",
///   "max_accounts": 500,
///   "max_rooms": 100,
///   "max_message_bytes": 4096,
///   "token_expiry_days": 7
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerInfo {
    /// The server build's version, as in its `Cargo.toml`.
    pub version: String,
    /// The SCION ISD-AS the server is reachable in, or `null` when it is serving over plain TCP
    /// in development mode.
    pub isd_as: Option<String>,
    /// How many accounts the server registers before it starts rejecting registrations.
    pub max_accounts: u32,
    /// How many rooms the server creates before it starts rejecting new ones.
    pub max_rooms: u32,
    /// The largest message body the server accepts, in bytes of UTF-8.
    pub max_message_bytes: u32,
    /// How long a token issued by `POST /api/v1/login` stays valid.
    pub token_expiry_days: u32,
}

/// The body of every failing response, whatever failed.
///
/// ```json
/// {
///   "error": {
///     "code": "room_not_found",
///     "message": "no room with id 7"
///   }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// What went wrong.
    pub error: ApiError,
}

impl ErrorResponse {
    /// Wraps a code and a message into the envelope.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: ApiError {
                code: code.into(),
                message: message.into(),
            },
        }
    }
}

/// The contents of an [`ErrorResponse`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    /// A stable, machine-readable identifier for the failure, in `snake_case` — for example
    /// `room_not_found` or `message_too_large`. This is what a client branches on; the HTTP
    /// status alone is too coarse.
    pub code: String,
    /// A human-readable explanation, for logs and for showing to a user. Free-form: it may change
    /// between server versions, so never branch on it.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use serde::de::DeserializeOwned;
    use serde_json::Value;

    use super::*;

    /// Asserts that `value` serializes to exactly `json`, and that `json` deserializes back into
    /// `value`.
    ///
    /// Every `json` below is copied verbatim from the corresponding type's doc example, so these
    /// tests fail as soon as a type stops matching what its documentation promises.
    #[track_caller]
    fn assert_wire_shape<T>(value: T, json: &str)
    where
        T: fmt::Debug + PartialEq + Serialize + DeserializeOwned,
    {
        let expected: Value = serde_json::from_str(json).expect("the example is valid JSON");
        assert_eq!(
            serde_json::to_value(&value).expect("serializing never fails"),
            expected,
            "serialized shape differs from the doc example"
        );
        assert_eq!(
            serde_json::from_value::<T>(expected).expect("the example decodes"),
            value,
            "the doc example decodes into something else"
        );
    }

    #[test]
    fn register_request() {
        assert_wire_shape(
            RegisterRequest {
                username: "alice".to_owned(),
                password: "correct horse battery staple".to_owned(),
            },
            r#"{
              "username": "alice",
              "password": "correct horse battery staple"
            }"#,
        );
    }

    #[test]
    fn login_request() {
        assert_wire_shape(
            LoginRequest {
                username: "alice".to_owned(),
                password: "correct horse battery staple".to_owned(),
            },
            r#"{
              "username": "alice",
              "password": "correct horse battery staple"
            }"#,
        );
    }

    #[test]
    fn login_response() {
        assert_wire_shape(
            LoginResponse {
                token: "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhbGljZSIsImV4cCI6MTc5MDAwMDAwMH0.c2ln"
                    .to_owned(),
                expires_at: 1_790_000_000_000,
            },
            r#"{
              "token": "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJhbGljZSIsImV4cCI6MTc5MDAwMDAwMH0.c2ln",
              "expires_at": 1790000000000
            }"#,
        );
    }

    #[test]
    fn room() {
        assert_wire_shape(
            Room {
                id: 1,
                name: "lobby".to_owned(),
                latest_seq: Some(42),
            },
            r#"{
              "id": 1,
              "name": "lobby",
              "latest_seq": 42
            }"#,
        );
    }

    /// A room nobody has posted in yet carries an explicit `null`, not a missing field: the
    /// mobile clients decode against a fixed set of keys.
    #[test]
    fn room_without_messages() {
        assert_wire_shape(
            Room {
                id: 2,
                name: "scion".to_owned(),
                latest_seq: None,
            },
            r#"{ "id": 2, "name": "scion", "latest_seq": null }"#,
        );
    }

    #[test]
    fn rooms_response() {
        assert_wire_shape(
            RoomsResponse {
                rooms: vec![
                    Room {
                        id: 1,
                        name: "lobby".to_owned(),
                        latest_seq: Some(42),
                    },
                    Room {
                        id: 2,
                        name: "scion".to_owned(),
                        latest_seq: None,
                    },
                ],
            },
            r#"{
              "rooms": [
                { "id": 1, "name": "lobby", "latest_seq": 42 },
                { "id": 2, "name": "scion", "latest_seq": null }
              ]
            }"#,
        );
    }

    #[test]
    fn create_room_request() {
        assert_wire_shape(
            CreateRoomRequest {
                name: "scion".to_owned(),
            },
            r#"{
              "name": "scion"
            }"#,
        );
    }

    #[test]
    fn post_message_request() {
        assert_wire_shape(
            PostMessageRequest {
                body: "hello from 1-ff00:0:110".to_owned(),
            },
            r#"{
              "body": "hello from 1-ff00:0:110"
            }"#,
        );
    }

    #[test]
    fn post_message_response() {
        assert_wire_shape(
            PostMessageResponse {
                seq: 43,
                posted_at: 1_789_994_400_000,
            },
            r#"{
              "seq": 43,
              "posted_at": 1789994400000
            }"#,
        );
    }

    #[test]
    fn message() {
        assert_wire_shape(
            Message {
                seq: 43,
                username: "alice".to_owned(),
                body: "hello from 1-ff00:0:110".to_owned(),
                posted_at: 1_789_994_400_000,
            },
            r#"{
              "seq": 43,
              "username": "alice",
              "body": "hello from 1-ff00:0:110",
              "posted_at": 1789994400000
            }"#,
        );
    }

    #[test]
    fn messages_response() {
        assert_wire_shape(
            MessagesResponse {
                messages: vec![
                    Message {
                        seq: 42,
                        username: "bob".to_owned(),
                        body: "anyone here?".to_owned(),
                        posted_at: 1_789_994_300_000,
                    },
                    Message {
                        seq: 43,
                        username: "alice".to_owned(),
                        body: "hello".to_owned(),
                        posted_at: 1_789_994_400_000,
                    },
                ],
            },
            r#"{
              "messages": [
                { "seq": 42, "username": "bob", "body": "anyone here?", "posted_at": 1789994300000 },
                { "seq": 43, "username": "alice", "body": "hello", "posted_at": 1789994400000 }
              ]
            }"#,
        );
    }

    /// The end of a room's history, and a poll that found nothing new, are the same empty page.
    #[test]
    fn messages_response_empty() {
        assert_wire_shape(
            MessagesResponse { messages: vec![] },
            r#"{ "messages": [] }"#,
        );
    }

    #[test]
    fn server_info() {
        assert_wire_shape(
            ServerInfo {
                version: "0.1.0".to_owned(),
                isd_as: Some("1-ff00:0:110".to_owned()),
                max_accounts: 500,
                max_rooms: 100,
                max_message_bytes: 4096,
                token_expiry_days: 7,
            },
            r#"{
              "version": "0.1.0",
              "isd_as": "1-ff00:0:110",
              "max_accounts": 500,
              "max_rooms": 100,
              "max_message_bytes": 4096,
              "token_expiry_days": 7
            }"#,
        );
    }

    /// Serving over plain TCP, the server has no SCION address to report.
    #[test]
    fn server_info_without_isd_as() {
        let json = r#"{
          "version": "0.1.0",
          "isd_as": null,
          "max_accounts": 500,
          "max_rooms": 100,
          "max_message_bytes": 4096,
          "token_expiry_days": 7
        }"#;
        assert_wire_shape(
            ServerInfo {
                version: "0.1.0".to_owned(),
                isd_as: None,
                max_accounts: 500,
                max_rooms: 100,
                max_message_bytes: 4096,
                token_expiry_days: 7,
            },
            json,
        );
    }

    #[test]
    fn error_response() {
        assert_wire_shape(
            ErrorResponse::new("room_not_found", "no room with id 7"),
            r#"{
              "error": {
                "code": "room_not_found",
                "message": "no room with id 7"
              }
            }"#,
        );
    }

    /// Decoding tolerates fields it does not know, so a client and a server built at different
    /// commits still exchange whatever they have in common.
    #[test]
    fn unknown_fields_are_ignored() {
        let from_a_newer_server = r#"{
          "id": 1,
          "name": "lobby",
          "latest_seq": 42,
          "topic": "a field this version has never heard of"
        }"#;
        let room: Room = serde_json::from_str(from_a_newer_server).expect("decodes");
        assert_eq!(room.name, "lobby");
    }

    /// Asserts that `value`'s `Debug` output hides `secret` behind the redaction marker while
    /// still showing `kept` — a field that is not a secret, so that redaction cannot be passed by
    /// rendering nothing at all.
    #[track_caller]
    fn assert_debug_redacts(value: impl fmt::Debug, secret: &str, kept: &str) {
        let rendered = format!("{value:?}");
        assert!(!rendered.contains(secret), "secret leaked: {rendered}");
        assert!(rendered.contains(REDACTED), "secret not marked: {rendered}");
        assert!(
            rendered.contains(kept),
            "non-secret field missing: {rendered}"
        );
    }

    /// Passwords and tokens must not reach a log through the `Debug` impl of the struct that
    /// carries them.
    #[test]
    fn secrets_are_redacted_in_debug_output() {
        assert_debug_redacts(
            RegisterRequest {
                username: "alice".to_owned(),
                password: "correct horse battery staple".to_owned(),
            },
            "correct horse battery staple",
            "alice",
        );
        assert_debug_redacts(
            LoginRequest {
                username: "alice".to_owned(),
                password: "correct horse battery staple".to_owned(),
            },
            "correct horse battery staple",
            "alice",
        );
        assert_debug_redacts(
            LoginResponse {
                token: "eyJhbGciOiJIUzI1NiJ9.c2ln".to_owned(),
                expires_at: 1_790_000_000_000,
            },
            "eyJhbGciOiJIUzI1NiJ9.c2ln",
            "1790000000000",
        );
    }
}
