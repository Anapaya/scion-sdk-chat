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

/// The server's version and the limits it enforces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ServerInfo {
    /// The server build's version.
    pub version: String,
    /// The SCION ISD-AS the server is reachable in. `null` while it serves over plain TCP.
    // TODO: replace with `sciparse::IsdAsn`. It serializes to this same string, is `Copy`, and
    // already carries a `ToSchema` with an example and a validation pattern — so the published
    // schema stops saying "some string". Costs this crate its first SDK dependency.
    pub isd_as: Option<String>,
    /// How many accounts are registered before registration is refused.
    pub max_accounts: u32,
    /// How many rooms are created before creation is refused.
    pub max_rooms: u32,
    /// The largest message body accepted, in bytes of UTF-8.
    pub max_message_bytes: u32,
    /// How long a token stays valid after it is issued.
    pub token_validity_seconds: u32,
}
