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
//! The one body every failing response carries.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// The body of every failing response, whatever failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    /// What went wrong.
    pub error: ApiError,
}

impl ErrorResponse {
    /// Wraps a code and a message into the envelope.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            error: ApiError {
                code,
                message: message.into(),
            },
        }
    }
}

/// The contents of an [`ErrorResponse`].
// Fields are private so that `ErrorResponse::new` is the only way to build one: a sender reaches
// the wire through a single door, and a receiver reads what arrived rather than editing it. Kept
// out of the doc comment, which utoipa publishes as the schema description.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ApiError {
    /// What failed, as a value a client can branch on. The HTTP status alone is too coarse.
    code: ErrorCode,
    /// A human-readable explanation, for logs and for showing to a user. Free-form: it may change
    /// between server versions, so never branch on it.
    message: String,
}

impl ApiError {
    /// What failed — the value to branch on.
    pub fn code(&self) -> &ErrorCode {
        &self.code
    }

    /// The human-readable explanation. Never branch on it.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Every failure this API distinguishes, on the wire as its name in `snake_case`.
///
/// An enum rather than a string so the server cannot emit a code that does not exist and a client
/// can `match` with the compiler checking it has covered them all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The request body was missing, malformed, or not the shape the endpoint expects.
    InvalidBody,
    /// The server failed for a reason it does not describe further.
    Internal,
    /// The request carried no usable bearer token.
    Unauthorized,
    /// The token was issued by this server but has passed its expiry. New login required.
    ExpiredToken,
    /// The username exists but the password does not match it.
    InvalidCredentials,
    /// The requested username is already registered.
    UsernameTaken,
    /// The requested username is outside what registration accepts.
    InvalidUsername,
    /// The requested room name is outside what room creation accepts.
    InvalidName,
    /// No room has the requested id.
    RoomNotFound,
    /// The message body exceeds the server's limit.
    MessageTooLarge,
    /// A configured cap — accounts or rooms — is already reached.
    CapExceeded,
    /// A code this build does not know, kept verbatim.
    ///
    /// Decoding an error must not itself fail: a client that meets a code added after it was
    /// built still surfaces the server's message rather than a parse error.
    #[serde(untagged)]
    Other(UnknownCode),
}

/// A code carried by a response but not named by [`ErrorCode`].
///
/// Its contents are private and it has no constructor, so decoding is the only thing that can
/// produce one. That is what makes a sender structurally unable to invent a code: reaching for
/// [`ErrorCode::Other`] instead of adding a variant is not an option it has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[schema(value_type = String)]
pub struct UnknownCode(String);

impl UnknownCode {
    /// The code exactly as it arrived, for logging or display.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
