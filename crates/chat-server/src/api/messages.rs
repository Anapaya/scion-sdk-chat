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
    ErrorCode, ErrorResponse, MessagesResponse, PostMessageRequest, PostMessageResponse, RoomId,
    Seq,
};
use serde::Deserialize;
use utoipa::IntoParams;

use super::{ApiError, AppState, auth::Caller};

/// The default page size, and the most a caller can ask for.
const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 200;

/// Which page of a room's messages to return.
///
/// The two cursors are mutually exclusive; passing both is a client error rather than a silent
/// preference for one of them.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct Page {
    /// How many messages to return, clamped to 1..=200. Defaults to 50.
    limit: Option<u32>,
    /// Return only messages newer than this `seq`.
    after_seq: Option<u64>,
    /// Return only messages older than this `seq`.
    before_seq: Option<u64>,
}

impl Page {
    /// Clamps `limit` into 1..=200, which bounds what one request can cost the server.
    fn limit(&self) -> u32 {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }
}

/// Read a page of a room's messages, oldest first.
#[utoipa::path(
    get,
    path = "/rooms/{id}/messages",
    params(("id" = u64, Path, description = "The room to read"), Page),
    responses(
        (status = 200, description = "A page of messages, oldest first", body = MessagesResponse),
        (status = 400, description = "Both cursors were given", body = ErrorResponse),
        (status = 401, description = "No usable bearer token", body = ErrorResponse),
        (status = 404, description = "No room with that id", body = ErrorResponse),
    ),
    security(("bearer" = [])),
    tag = "messages",
)]
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

/// Append a message to a room, attributed to the token holder.
#[utoipa::path(
    post,
    path = "/rooms/{id}/messages",
    params(("id" = u64, Path, description = "The room to post to")),
    request_body = PostMessageRequest,
    responses(
        (status = 201, description = "Where the message landed", body = PostMessageResponse),
        (status = 401, description = "No usable bearer token", body = ErrorResponse),
        (status = 404, description = "No room with that id", body = ErrorResponse),
        (status = 413, description = "The body is larger than the server accepts", body = ErrorResponse),
        (status = 422, description = "The body is empty", body = ErrorResponse),
    ),
    security(("bearer" = [])),
    tag = "messages",
)]
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
