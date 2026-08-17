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
//! Listing and creating rooms.

use std::sync::Arc;

use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use chat_core::api::v1::{CreateRoomRequest, ErrorCode, ErrorResponse, Room, RoomsResponse};

use super::{API_V1, ApiError, AppState, auth::Authenticated};
use crate::store::RoomCreation;

/// A room name, once it has been checked.
struct RoomName(String);

impl RoomName {
    /// Accepts 1–64 printable ASCII characters, matching what the wire contract documents.
    fn parse(raw: &str) -> Result<Self, ApiError> {
        let printable = raw.chars().all(|c| c.is_ascii_graphic() || c == ' ');

        if (1..=64).contains(&raw.len()) && printable {
            Ok(Self(raw.to_owned()))
        } else {
            Err(ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorCode::InvalidName,
                "a room name is 1 to 64 printable ASCII characters",
            ))
        }
    }
}

/// List every room.
#[utoipa::path(
    get,
    path = "/rooms",
    responses(
        (status = 200, description = "Every room on the server", body = RoomsResponse),
        (status = 401, description = "No usable bearer token", body = ErrorResponse),
    ),
    security(("bearer" = [])),
    tag = "rooms",
)]
pub async fn list(
    _: Authenticated,
    State(state): State<Arc<AppState>>,
) -> Result<Json<RoomsResponse>, ApiError> {
    let rooms = state.store.list_rooms().await?;

    Ok(Json(RoomsResponse { rooms }))
}

/// Create a room, or return the one already holding the name.
///
/// Creation is idempotent on the name, so a client never has to look the id up separately.
#[utoipa::path(
    post,
    path = "/rooms",
    request_body = CreateRoomRequest,
    responses(
        (status = 201, description = "The room was created", body = Room,
            headers(("location" = String, description = "Where the new room is"))),
        (status = 200, description = "The name was taken; this is the room holding it", body = Room),
        (status = 401, description = "No usable bearer token", body = ErrorResponse),
        (status = 422, description = "The name is not acceptable", body = ErrorResponse),
        (status = 429, description = "The server accepts no more rooms", body = ErrorResponse),
    ),
    security(("bearer" = [])),
    tag = "rooms",
)]
pub async fn create(
    _: Authenticated,
    State(state): State<Arc<AppState>>,
    body: Result<Json<CreateRoomRequest>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(body) = body?;
    let name = RoomName::parse(&body.name)?;

    match state.store.create_room(&name.0).await? {
        RoomCreation::Created(room) => Ok(created(room)),
        RoomCreation::Existing(room) => Ok((StatusCode::OK, Json(room)).into_response()),
    }
}

/// A 201 carries the location of what it created, which a 200 has no reason to.
fn created(room: Room) -> Response {
    let location = format!("{API_V1}/rooms/{}", room.id);

    (
        StatusCode::CREATED,
        [(header::LOCATION, location)],
        Json(room),
    )
        .into_response()
}
