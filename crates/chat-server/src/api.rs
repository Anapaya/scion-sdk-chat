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
//! The HTTP API: the router, the state it carries, and how failures become responses.

use std::sync::Arc;

use axum::{
    Json, Router,
    extract::rejection::JsonRejection,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use chat_core::api::v1::{ErrorCode, ErrorResponse};
use utoipa::{
    Modify, OpenApi,
    openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    auth::{AuthError, Tokens},
    config::Config,
    store::{DataStore, StoreError},
};

mod auth;
mod messages;
mod rooms;
mod server;
#[cfg(test)]
mod tests;

/// The prefix every route sits under.
pub const API_V1: &str = "/api/v1";

/// What every handler is given.
///
/// Shared behind an [`Arc`] rather than cloned per request, so the fields are plain values.
pub struct AppState {
    /// Where chat data lives.
    pub store: Box<dyn DataStore>,
    /// Issues and validates bearer tokens.
    pub tokens: Tokens,
    /// The caps and limits the API enforces and reports.
    pub config: Config,
}

/// Where the generated OpenAPI document is served.
pub const OPENAPI_PATH: &str = "/.well-known/openapi.json";

/// The document's title, version and security scheme. The paths and schemas come from the
/// handlers themselves, so this holds only what they cannot say.
#[derive(OpenApi)]
#[openapi(
    info(title = "scion-chat", description = "A chat server over HTTP/3-over-SCION"),
    modifiers(&BearerAuth),
    tags(
        (name = "server", description = "Liveness and server metadata"),
        (name = "accounts", description = "Registering and logging in"),
        (name = "rooms", description = "Listing and creating rooms"),
        (name = "messages", description = "Posting and reading messages"),
    ),
)]
pub struct ApiDoc;

/// Declares the bearer scheme the authenticated paths refer to by name.
struct BearerAuth;

impl Modify for BearerAuth {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}

/// Every route, paired with the document describing it, so neither can be added without the other.
fn routes() -> OpenApiRouter<Arc<AppState>> {
    OpenApiRouter::with_openapi(ApiDoc::openapi()).nest(
        API_V1,
        OpenApiRouter::new()
            .routes(routes!(server::healthz))
            .routes(routes!(server::server_info))
            .routes(routes!(auth::register))
            .routes(routes!(auth::login))
            .routes(routes!(rooms::list, rooms::create))
            .routes(routes!(messages::list, messages::post)),
    )
}

/// Builds the router. Transport-agnostic: the same value is served over TCP or over SCION.
pub fn router(state: AppState) -> Router {
    let (router, spec) = routes().with_state(Arc::new(state)).split_for_parts();

    router.route(OPENAPI_PATH, get(async || Json(spec)))
}

/// The document describing the API, without the state a served router needs.
pub fn openapi() -> utoipa::openapi::OpenApi {
    routes().split_for_parts().1
}

/// A failure on its way to becoming a response.
///
/// Handlers return this rather than a status code, so that every failure carries the envelope and
/// the code a client branches on.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: ErrorCode,
    message: String,
}

impl ApiError {
    /// Builds a failure from the status a client sees and the code it branches on.
    pub fn new(status: StatusCode, code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    /// The request carried no usable bearer token.
    pub fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            ErrorCode::Unauthorized,
            "a valid bearer token is required",
        )
    }

    /// The server failed for a reason the caller is not told.
    pub fn internal() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Internal,
            "the server failed to handle this request",
        )
    }

    /// No room has the requested id.
    pub fn room_not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            ErrorCode::RoomNotFound,
            "no room with that id",
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorResponse::new(self.code, self.message)),
        )
            .into_response()
    }
}

/// Nothing a caller did wrong, so nothing a caller can act on: the detail is logged and the
/// response says only that the server failed.
impl From<StoreError> for ApiError {
    fn from(error: StoreError) -> Self {
        match error {
            // The only row the store reports missing is a room.
            StoreError::NotFound(_) => Self::room_not_found(),
            StoreError::CapExceeded { what } => {
                Self::new(
                    StatusCode::TOO_MANY_REQUESTS,
                    ErrorCode::CapExceeded,
                    format!("this server accepts no more {what}s"),
                )
            }
            error => {
                tracing::error!(%error, "store failed");
                Self::internal()
            }
        }
    }
}

impl From<AuthError> for ApiError {
    fn from(error: AuthError) -> Self {
        tracing::error!(%error, "auth failed");
        Self::internal()
    }
}

/// A malformed or missing JSON body is a client error, reported in the same envelope as the rest.
impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            ErrorCode::InvalidBody,
            rejection.body_text(),
        )
    }
}
