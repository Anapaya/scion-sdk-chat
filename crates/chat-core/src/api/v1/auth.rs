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
//! Registering an account and exchanging a password for a bearer token.

use std::fmt;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::UnixMillis;

/// Stands in for a secret in `Debug` output, so a printed struct cannot leak one.
const REDACTED: &str = "<redacted>";

/// The credentials a new account is created with.
///
/// The username is the account's permanent identity — the name messages are attributed to. There
/// is no rename, no password change, and no password reset.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct RegisterRequest {
    /// The name to register. UTF-8, 1–32 characters, no control characters; compared
    /// case-insensitively against the names already taken.
    pub username: String,
    /// The password in the clear, protected by the connection's TLS. Only a KDF hash is kept.
    pub password: String,
}

impl fmt::Debug for RegisterRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisterRequest")
            .field("username", &self.username)
            .field("password", &REDACTED)
            .finish()
    }
}

/// The credentials an existing account is authenticated with.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct LoginRequest {
    /// The registered name to log in as.
    pub username: String,
    /// The password in the clear, verified against the stored hash.
    pub password: String,
}

impl fmt::Debug for LoginRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginRequest")
            .field("username", &self.username)
            .field("password", &REDACTED)
            .finish()
    }
}

/// What a successful login yields: the bearer token every authenticated request carries.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct LoginResponse {
    /// The JWT to send as `Authorization: Bearer <token>`. Opaque: carry it, do not parse it.
    pub token: String,
    /// When the token stops being accepted. No refresh tokens: a client logs in again.
    pub expires_at: UnixMillis,
}

impl fmt::Debug for LoginResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginResponse")
            .field("token", &REDACTED)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Asserts `Debug` hides `secret` and still shows `kept`, so rendering nothing cannot pass.
    #[track_caller]
    fn assert_debug_redacts(value: impl fmt::Debug, secret: &str, kept: &str) {
        let rendered = format!("{value:?}");
        assert!(!rendered.contains(secret), "secret leaked: {rendered}");
        assert!(rendered.contains(REDACTED), "secret not marked: {rendered}");
        assert!(
            rendered.contains(kept),
            "non-secret field missing: {rendered}"
        );
    }

    /// Passwords and tokens must not reach a log through the `Debug` impl of the struct that
    /// carries them.
    #[test]
    fn secrets_are_redacted_in_debug_output() {
        assert_debug_redacts(
            RegisterRequest {
                username: "alice".to_owned(),
                password: "correct horse battery staple".to_owned(),
            },
            "correct horse battery staple",
            "alice",
        );
        assert_debug_redacts(
            LoginRequest {
                username: "alice".to_owned(),
                password: "correct horse battery staple".to_owned(),
            },
            "correct horse battery staple",
            "alice",
        );
        assert_debug_redacts(
            LoginResponse {
                token: "eyJhbGciOiJIUzI1NiJ9.c2ln".to_owned(),
                expires_at: UnixMillis::new(1_790_000_000_000),
            },
            "eyJhbGciOiJIUzI1NiJ9.c2ln",
            "1790000000000",
        );
    }
}
