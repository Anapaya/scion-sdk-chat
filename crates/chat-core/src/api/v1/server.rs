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
//! What the server says about itself.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Everything about the server a client may want to show or adapt to. Needs no token, so a
/// client can read it before anyone logs in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
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
    /// How long a token issued at login stays valid.
    pub token_expiry_days: u32,
}
