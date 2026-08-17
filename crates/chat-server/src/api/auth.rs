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
//! Registering an account, logging in, and authenticating every other request.

use std::sync::Arc;

use axum::{
    Json,
    extract::{FromRequestParts, State},
    http::{StatusCode, request::Parts},
};
use chat_core::api::v1::{ErrorCode, ErrorResponse, LoginRequest, LoginResponse, RegisterRequest};

use super::{ApiError, AppState};
use crate::{
    auth::{hash_password, verify_password},
    store::Registration,
};

/// A username, once it has been checked.
///
/// Handlers take this rather than a `String`, so a request cannot reach the store without the
/// name having been validated on the way in.
pub struct Username(pub String);

impl Username {
    /// Accepts 1–32 characters with nothing unprintable in them.
    fn parse(raw: &str) -> Result<Self, ApiError> {
        let length = raw.chars().count();
        let printable = !raw.chars().any(char::is_control);

        if (1..=32).contains(&length) && printable {
            Ok(Self(raw.to_owned()))
        } else {
            Err(ApiError::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                ErrorCode::InvalidUsername,
                "a username is 1 to 32 characters and holds no control characters",
            ))
        }
    }
}

/// The username the bearer token names.
///
/// Any handler that takes this argument is authenticated by construction: axum runs the
/// extraction before the handler body, and a missing or invalid token never reaches it.
pub struct Authenticated(pub String);

impl FromRequestParts<Arc<AppState>> for Authenticated {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split_once(' '))
            .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("Bearer"))
            .map(|(_, token)| token.trim())
            .ok_or_else(ApiError::unauthorized)?;

        let username = state.tokens.verify(token).map_err(|error| {
            if error.is_expired_token() {
                ApiError::expired_token()
            } else {
                ApiError::unauthorized()
            }
        })?;

        Ok(Self(username))
    }
}

/// Register an account.
#[utoipa::path(
    post,
    path = "/register",
    request_body = RegisterRequest,
    responses(
        (status = 201, description = "The account was created"),
        (status = 409, description = "The username is taken", body = ErrorResponse),
        (status = 422, description = "The username is not acceptable", body = ErrorResponse),
        (status = 429, description = "The server accepts no more accounts", body = ErrorResponse),
    ),
    tag = "accounts",
)]
pub async fn register(
    State(state): State<Arc<AppState>>,
    body: Result<Json<RegisterRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let Json(body) = body?;
    let username = Username::parse(&body.username)?;

    // Hashing is deliberately slow, so it runs on a thread that is allowed to block.
    let password = body.password.clone();
    let hash = tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .expect("the hashing task cannot panic")?;

    match state.store.insert_user(&username.0, &hash).await {
        Ok(Registration::Created) => Ok(StatusCode::CREATED),
        Ok(Registration::UsernameTaken) => {
            Err(ApiError::new(
                StatusCode::CONFLICT,
                ErrorCode::UsernameTaken,
                "that username is already registered",
            ))
        }
        Err(e) => Err(e.into()),
    }
}

/// Exchange a username and password for a bearer token.
#[utoipa::path(
    post,
    path = "/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "The token to send as `Authorization: Bearer`", body = LoginResponse),
        (status = 401, description = "The username and password do not match", body = ErrorResponse),
    ),
    tag = "accounts",
)]
pub async fn login(
    State(state): State<Arc<AppState>>,
    body: Result<Json<LoginRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<LoginResponse>, ApiError> {
    let Json(body) = body?;

    // Look the account up whether or not the name is well-formed, so that a rejected username
    // and a wrong password are indistinguishable from the outside.
    let stored = state.store.password_hash(&body.username).await?;

    let password = body.password.clone();
    let verified = tokio::task::spawn_blocking(move || verify_password(&password, stored.as_ref()))
        .await
        .expect("the verification task cannot panic");

    if !verified {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            ErrorCode::InvalidCredentials,
            "that username and password do not match",
        ));
    }

    let (token, expires_at) = state.tokens.issue(&body.username)?;
    Ok(Json(LoginResponse { token, expires_at }))
}
