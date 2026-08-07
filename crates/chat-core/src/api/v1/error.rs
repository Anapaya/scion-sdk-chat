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
///
/// ```json
/// {
///   "error": {
///     "code": "room_not_found",
///     "message": "no room with id 7"
///   }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    /// What went wrong.
    pub error: ApiError,
}

impl ErrorResponse {
    /// Wraps a code and a message into the envelope.
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: ApiError {
                code: code.into(),
                message: message.into(),
            },
        }
    }
}

/// The contents of an [`ErrorResponse`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ApiError {
    /// A stable, machine-readable identifier for the failure, in `snake_case` — for example
    /// `room_not_found` or `message_too_large`. This is what a client branches on; the HTTP
    /// status alone is too coarse.
    pub code: String,
    /// A human-readable explanation, for logs and for showing to a user. Free-form: it may change
    /// between server versions, so never branch on it.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::{super::test_support::assert_wire_shape, *};

    #[test]
    fn error_response() {
        assert_wire_shape(
            ErrorResponse::new("room_not_found", "no room with id 7"),
            r#"{
              "error": {
                "code": "room_not_found",
                "message": "no room with id 7"
              }
            }"#,
        );
    }
}
