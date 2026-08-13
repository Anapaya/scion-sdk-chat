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
//! Posting and reading messages.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chat_core::api::v1::{
    ErrorCode, MessagesResponse, PostMessageRequest, PostMessageResponse, RoomId, Seq,
};
use serde::Deserialize;

use super::{ApiError, AppState, auth::Caller};

/// The default page size, and the most a caller can ask for.
const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 200;

/// Which page of a room's messages to return.
///
/// The two cursors are mutually exclusive; passing both is a client error rather than a silent
/// preference for one of them.
#[derive(Debug, Deserialize)]
pub struct Page {
    limit: Option<u32>,
    after_seq: Option<u64>,
    before_seq: Option<u64>,
}

impl Page {
    /// Clamps `limit` into 1..=200, which bounds what one request can cost the server.
    fn limit(&self) -> u32 {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }
}

/// `GET /rooms/{id}/messages`
pub async fn list(
    _: Caller,
    State(state): State<Arc<AppState>>,
    Path(room): Path<u64>,
    Query(page): Query<Page>,
) -> Result<Json<MessagesResponse>, ApiError> {
    let room = RoomId::new(room);
    if !state.store.room_exists(room).await? {
        return Err(ApiError::room_not_found());
    }

    let limit = page.limit();
    let messages = match (page.after_seq, page.before_seq) {
        (Some(_), Some(_)) => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                ErrorCode::InvalidBody,
                "after_seq and before_seq cannot be combined",
            ));
        }
        (Some(after), None) => {
            state
                .store
                .messages_after(room, Seq::new(after), limit)
                .await?
        }
        (None, Some(before)) => {
            state
                .store
                .messages_before(room, Seq::new(before), limit)
                .await?
        }
        (None, None) => state.store.messages_newest(room, limit).await?,
    };

    Ok(Json(MessagesResponse { messages }))
}

/// `POST /rooms/{id}/messages`
pub async fn post(
    Caller(username): Caller,
    State(state): State<Arc<AppState>>,
    Path(room): Path<u64>,
    body: Result<Json<PostMessageRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(body) = body?;
    let room = RoomId::new(room);

    if body.body.len() > state.config.max_message_bytes as usize {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            ErrorCode::MessageTooLarge,
            format!(
                "a message body is at most {} bytes",
                state.config.max_message_bytes
            ),
        ));
    }
    if body.body.is_empty() {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidBody,
            "a message body cannot be empty",
        ));
    }

    let posted: PostMessageResponse = state
        .store
        .post_message(room, &username, &body.body)
        .await?;

    Ok((StatusCode::CREATED, Json(posted)).into_response())
}
