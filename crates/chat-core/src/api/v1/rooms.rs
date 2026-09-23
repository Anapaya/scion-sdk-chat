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

/// A room. Every user is in every room, and one named `lobby` always exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Room {
    /// The room's identifier, as the message paths take it.
    pub id: RoomId,
    /// The room's display name. Printable ASCII, 1–64 characters; unique case-insensitively.
    pub name: String,
    /// The `seq` of the newest message in the room, or `0` while it has none.
    ///
    /// Against the newest `seq` a client has read, this drives an unread badge. Never a count:
    /// `seq` is server-wide (see [`Seq`]).
    pub latest_seq: Seq,
}

/// Every room on the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RoomsResponse {
    /// Never empty: `lobby` is always present.
    pub rooms: Vec<Room>,
}

/// The name a new room is created under.
///
/// Creation is idempotent on the name: an existing name returns that room instead of failing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CreateRoomRequest {
    /// The name to create. Printable ASCII, 1–64 characters; matched case-insensitively against
    /// the rooms that already exist.
    pub name: String,
}
