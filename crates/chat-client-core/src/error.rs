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
//! What a call can fail with.

use chat_core::api::v1::ErrorCode;

/// Anything a client method can fail with. The one error type a user interface sees.
#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    /// The configuration cannot be used: a bad address, a missing certificate.
    #[error("configuration: {0}")]
    Config(String),
    /// The request did not complete.
    #[error(transparent)]
    Transport(#[from] TransportError),
    /// The server refused the request and said why.
    #[error("{code:?} ({status}): {message}")]
    Api {
        /// The HTTP status that carried the refusal.
        status: u16,
        /// What failed, as the value to branch on.
        code: ErrorCode,
        /// The server's explanation. Free-form: never branch on it.
        message: String,
    },
    /// A reply arrived that could not be decoded.
    #[error("the server sent a reply this client cannot read: {0}")]
    Protocol(String),
    /// The call needs a token and nobody has logged in.
    #[error("no one is logged in")]
    NotLoggedIn,
    /// The token was refused. Only the user can fix this, by logging in again.
    #[error("the session has ended; log in again")]
    SessionExpired,
}

/// Why a request never came back. Mirrors the taxonomy of the SDK's HTTP/3 client, so both
/// transports report the same kinds of failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TransportError {
    /// The server's name could not be resolved.
    #[error("cannot resolve the server: {0}")]
    Resolution(String),
    /// No connection could be established.
    #[error("cannot reach the server: {0}")]
    Connect(String),
    /// The connection was refused on trust grounds.
    #[error("the server's identity was refused: {0}")]
    Tls(String),
    /// The connection dropped mid-request.
    #[error("the connection dropped: {0}")]
    StreamReset(String),
    /// The exchange did not follow HTTP.
    #[error("the exchange did not follow HTTP: {0}")]
    Protocol(String),
    /// The reply was larger than this client accepts.
    #[error("the reply exceeds the {limit} byte limit")]
    BodyTooLarge {
        /// The largest reply that would have been read.
        limit: usize,
    },
    /// The request took too long.
    #[error("the server did not answer in time")]
    Timeout,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A transport failure reaches the caller without being reworded on the way.
    #[test]
    fn a_transport_failure_converts_without_losing_which_one_it_was() {
        let error: ChatError = TransportError::Timeout.into();

        assert!(matches!(
            error,
            ChatError::Transport(TransportError::Timeout)
        ));
    }
}
