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
//! The contents land in follow-up tickets; this crate currently only reserves the workspace
//! slot.
