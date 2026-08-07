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
//! Listing the rooms on the server, and creating one.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{RoomId, Seq};

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreateRoomRequest {
    /// The name to create. UTF-8, 1–64 characters, no control characters; matched
    /// case-insensitively against the rooms that already exist.
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::{super::test_support::assert_wire_shape, *};

    /// An array of objects carrying a nullable field in both states — the shape where a
    /// hand-written schema and serde's output are likeliest to disagree. A room nobody has posted
    /// in yet carries an explicit `null` rather than dropping the key.
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
}
