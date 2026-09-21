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
//! Errors the client returns.

use chat_core::api::v1::{ErrorCode, ErrorResponse};

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
    /// The token was refused, and the client has forgotten it. Logging in again is the fix.
    #[error("the session has ended; log in again")]
    SessionExpired,
}

/// Why a request never came back. The SDK's own taxonomy, so both transports report alike.
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

/// Turns a refused reply into the error a caller sees.
///
/// A refusal outside the error envelope is the server breaking its contract, so it is a protocol
/// failure.
pub(crate) fn refusal(status: u16, body: &[u8]) -> ChatError {
    match serde_json::from_slice::<ErrorResponse>(body) {
        Ok(envelope) => {
            ChatError::Api {
                status,
                code: envelope.error.code().clone(),
                message: envelope.error.message().to_owned(),
            }
        }
        Err(error) => ChatError::Protocol(format!("{status} carried no error envelope: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A refusal the server described reaches the caller as the server described it.
    #[test]
    fn an_envelope_becomes_the_code_and_message_it_carries() {
        let body = br#"{"error":{"code":"room_not_found","message":"no room with that id"}}"#;

        let error = refusal(404, body);

        let ChatError::Api {
            status,
            code,
            message,
        } = error
        else {
            panic!("expected an api error, got {error:?}");
        };
        assert_eq!(status, 404);
        assert_eq!(code, ErrorCode::RoomNotFound);
        assert_eq!(message, "no room with that id");
    }

    /// A code added after this client was built still decodes.
    #[test]
    fn a_code_this_build_does_not_know_still_arrives() {
        let body = br#"{"error":{"code":"teapot","message":"short and stout"}}"#;

        let error = refusal(418, body);

        let ChatError::Api { code, .. } = error else {
            panic!("expected an api error, got {error:?}");
        };
        let ErrorCode::Other(unknown) = code else {
            panic!("expected an unknown code, got {code:?}");
        };
        assert_eq!(unknown.as_str(), "teapot");
    }

    /// A 401 is read like any other refusal: only the caller knows whether a token was sent.
    #[test]
    fn a_401_is_decoded_rather_than_assumed_to_be_an_ended_session() {
        let body = br#"{"error":{"code":"invalid_credentials","message":"no match"}}"#;

        let error = refusal(401, body);

        let ChatError::Api { status, code, .. } = error else {
            panic!("expected an api error, got {error:?}");
        };
        assert_eq!(status, 401);
        assert_eq!(code, ErrorCode::InvalidCredentials);
    }

    /// A refusal outside the envelope is reported as a protocol failure.
    #[test]
    fn a_refusal_without_an_envelope_is_a_protocol_failure() {
        for body in [b"{}".as_slice(), b"", b"not json"] {
            let error = refusal(502, body);

            assert!(
                matches!(error, ChatError::Protocol(_)),
                "expected a protocol failure, got {error:?}",
            );
        }
    }

    /// A transport failure reaches the caller unreworded.
    #[test]
    fn a_transport_failure_converts_without_losing_which_one_it_was() {
        let error: ChatError = TransportError::Timeout.into();

        assert!(matches!(
            error,
            ChatError::Transport(TransportError::Timeout)
        ));
    }
}
