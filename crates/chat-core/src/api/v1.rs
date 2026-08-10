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
//! Version 1 of the chat API: every JSON body it accepts or returns, and the error envelope it
//! fails with, one module per domain — [`auth`], [`rooms`], [`messages`], [`server`], [`error`].
//!
//! The types describe the bodies only, not the endpoints that carry them.
//!
//! The conventions, which hold for all of them:
//!
//! - **JSON only**, with `snake_case` field names, under `/api/v1`.
//! - **Timestamps are `unix_millis`** ([`UnixMillis`]) — integers, UTC.
//! - **`seq` is a JSON number** ([`Seq`]), and a cursor rather than a count.
//! - **Every failure has the same body**, [`ErrorResponse`], whatever the status code.

use std::fmt;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub mod auth;
pub mod error;
pub mod messages;
pub mod rooms;
pub mod server;

pub use auth::{LoginRequest, LoginResponse, RegisterRequest};
pub use error::{ApiError, ErrorCode, ErrorResponse, UnknownCode};
pub use messages::{Message, MessagesResponse, PostMessageRequest, PostMessageResponse};
pub use rooms::{CreateRoomRequest, Room, RoomsResponse};
pub use server::ServerInfo;

/// Identifier of a room, assigned by the server when the room is created. Stable for the room's
/// lifetime, and rooms are never deleted.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
#[serde(transparent)]
#[schema(value_type = i64)]
pub struct RoomId(i64);

impl RoomId {
    /// Wraps the value the server assigned.
    pub const fn new(id: i64) -> Self {
        Self(id)
    }

    /// The value beneath, for storage and for building request paths.
    pub const fn get(self) -> i64 {
        self.0
    }
}

impl fmt::Display for RoomId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Position of a message in the server's message sequence.
///
/// `seq` increases strictly and never regresses, so it is the one total order that every client
/// agrees on. It is assigned server-wide rather than per room, so a single room's messages carry
/// gaps: treat `seq` as a cursor, never as a count. Clients remember the highest `seq` they have
/// seen in a room and poll for what came after it.
///
/// Numbering starts at 1, so [`Seq::START`] is the position before every message — an empty room
/// and a client that has read nothing both sit there, and asking for what follows it asks for
/// everything.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
#[serde(transparent)]
#[schema(value_type = i64)]
pub struct Seq(i64);

impl Seq {
    /// The position before every message, which no message ever occupies.
    pub const START: Self = Self(0);

    /// Wraps the value the server assigned.
    pub const fn new(seq: i64) -> Self {
        Self(seq)
    }

    /// The value beneath, for storage and for building query strings.
    pub const fn get(self) -> i64 {
        self.0
    }
}

impl fmt::Display for Seq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A point in time on the wire: milliseconds since the Unix epoch, UTC.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
)]
#[serde(transparent)]
#[schema(value_type = i64)]
pub struct UnixMillis(i64);

impl UnixMillis {
    /// Wraps a count of milliseconds since the Unix epoch.
    pub const fn new(millis: i64) -> Self {
        Self(millis)
    }

    /// The count of milliseconds beneath, for storage and for conversion to a date type.
    pub const fn get(self) -> i64 {
        self.0
    }
}
