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
//! Posting a message to a room, and fetching a page of them.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{Seq, UnixMillis};

/// The text of a message being posted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PostMessageRequest {
    /// The message text. UTF-8, at most the server's `max_message_bytes` when encoded — see
    /// [`ServerInfo`](super::ServerInfo).
    pub body: String,
}

/// Where a posted message landed.
///
/// A client does *not* advance its poll cursor to this `seq` — a message with a lower `seq` may
/// still be waiting to be polled. The cursor only ever follows what polling actually delivered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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

/// A page of a room's messages, whichever of the three ways it was asked for: no cursor (the
/// newest page, to open a room), `after_seq` (poll, then append), or `before_seq` (load more,
/// then prepend).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct MessagesResponse {
    /// The page, **always oldest-first**, whichever cursor asked for it.
    ///
    /// A page shorter than the requested `limit` means the client has reached the end it was
    /// walking towards: the present when polling forwards, the start of history when loading
    /// older messages. A full page means more might be waiting — ask again immediately.
    pub messages: Vec<Message>,
}
