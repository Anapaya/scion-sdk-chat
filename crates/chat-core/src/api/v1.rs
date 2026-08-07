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
//! The types are plain data — no logic, no validation, no defaults. Each carries the JSON it
//! serializes to, so a client author on a platform without these structs (the Kotlin and Swift
//! apps) can implement against the doc comments alone.
//!
//! Each also derives [`utoipa::ToSchema`], so the same types describe themselves in the OpenAPI
//! document the server publishes. That derive is independent of serde's, and the two can
//! disagree — nullability and flattening being the usual places — so the shapes where they are
//! likeliest to diverge are covered by tests in the modules below.
//!
//! The conventions, which hold for all of them:
//!
//! - **JSON only**, with `snake_case` field names, under `/api/v1`.
//! - **Timestamps are `unix_millis`** ([`UnixMillis`]) — integers, UTC.
//! - **`seq` is a JSON number** ([`Seq`]), and a cursor rather than a count.
//! - **Every failure has the same body**, [`ErrorResponse`], whatever the status code.
//! - **Unknown fields are ignored**, in both directions — a decoder built at one commit still reads
//!   whatever it recognizes from a peer built at another.

pub mod auth;
pub mod error;
pub mod messages;
pub mod rooms;
pub mod server;
#[cfg(test)]
mod test_support;

pub use auth::{LoginRequest, LoginResponse, RegisterRequest};
pub use error::{ApiError, ErrorResponse};
pub use messages::{Message, MessagesResponse, PostMessageRequest, PostMessageResponse};
pub use rooms::{CreateRoomRequest, Room, RoomsResponse};
pub use server::ServerInfo;

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
