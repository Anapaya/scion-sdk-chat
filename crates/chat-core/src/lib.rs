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
//! Vocabulary shared by the chat server and every chat client: the wire contract (request and
//! response types plus the error envelope), and anything else the two sides must agree on
//! word-for-word — the error codes, the protocol limits and defaults, and validation of the
//! values the API accepts.
//!
//! Everything depends on this crate: the server, the clients, the end-to-end harness, and
//! eventually the mobile bindings. That stays affordable only while the crate is cheap to
//! depend on, so it holds types, constants and pure functions over them — nothing that performs
//! I/O, nothing async, and no dependency beyond serde. Code that does not meet that bar belongs
//! to whichever side needs it.
//!
//! # The wire contract
//!
//! The types below are every JSON body the API accepts or returns. Their conventions, which hold
//! for all of them:
//!
//! - **JSON only**, with `snake_case` field names, over `/api/v1`.
//! - **Timestamps are `unix_millis`** ([`UnixMillis`]) — integers, UTC, always the server's clock.
//! - **`seq` is a JSON number** ([`Seq`]), and a cursor rather than a count.
//! - **Every failure has the same body**, [`ErrorResponse`], whatever the status code.
//! - **Unknown fields are ignored**, in both directions. That is what makes the compatibility
//!   stance below survive a client and a server of different ages talking to each other.
//!
//! Compatibility: once a client ships, this contract only grows — new optional fields, never a
//! renamed, retyped or removed one. Anything else is a new endpoint.
//!
//! The protocol limits, the error codes and the validation rules join the crate with the tickets
//! that first enforce them.

mod wire;

pub use wire::{
    ApiError, CreateRoomRequest, ErrorResponse, LoginRequest, LoginResponse, Message,
    MessagesResponse, PostMessageRequest, PostMessageResponse, RegisterRequest, Room, RoomId,
    RoomsResponse, Seq, ServerInfo, UnixMillis,
};
