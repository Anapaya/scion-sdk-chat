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
//! What the server says about itself. Neither endpoint requires a token.

use std::sync::Arc;

use axum::{Json, extract::State};
use chat_core::api::v1::ServerInfo;
use serde::Serialize;
use utoipa::ToSchema;

use super::AppState;

/// The body of a liveness check.
#[derive(Debug, Serialize, ToSchema)]
pub struct Health {
    /// Always `ok`; the status code carries the answer.
    pub status: &'static str,
}

/// Report that the server is running.
#[utoipa::path(
    get,
    path = "/healthz",
    responses((status = 200, description = "The server is running", body = Health)),
    tag = "server",
)]
pub async fn healthz() -> Json<Health> {
    Json(Health { status: "ok" })
}

/// Report the version and the limits this server enforces.
#[utoipa::path(
    get,
    path = "/server",
    responses((status = 200, description = "Server metadata", body = ServerInfo)),
    tag = "server",
)]
pub async fn server_info(State(state): State<Arc<AppState>>) -> Json<ServerInfo> {
    Json(ServerInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        // Only the SCION transport knows an address to report.
        isd_as: None,
        max_accounts: state.config.max_accounts,
        max_rooms: state.config.max_rooms,
        max_message_bytes: state.config.max_message_bytes,
        token_validity_seconds: state
            .config
            .token_validity()
            .as_secs()
            .try_into()
            .unwrap_or(u32::MAX),
    })
}
