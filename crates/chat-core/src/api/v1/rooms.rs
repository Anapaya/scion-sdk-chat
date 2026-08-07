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

/// A room.
///
/// Every user belongs to every room: there is no membership, no joining, and no private rooms.
/// A room named `lobby` always exists.
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
    /// so one listing covers every room at once. It is not a message count:
    /// `seq` is server-wide (see [`Seq`]).
    pub latest_seq: Option<Seq>,
}

/// Every room on the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RoomsResponse {
    /// All rooms. Never empty: `lobby` is always present.
    pub rooms: Vec<Room>,
}

/// The name a new room is created under.
///
/// Creation is idempotent on the name: an existing name returns that room instead of failing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreateRoomRequest {
    /// The name to create. UTF-8, 1–64 characters, no control characters; matched
    /// case-insensitively against the rooms that already exist.
    pub name: String,
}
