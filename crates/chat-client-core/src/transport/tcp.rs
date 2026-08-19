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
//! Plain HTTP over TCP. Development only: no TLS, so nothing on the wire is protected.

use std::{error::Error as _, time::Duration};

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use reqwest::redirect::Policy;

use super::{MAX_BODY_BYTES, Transport};
use crate::error::{ChatError, TransportError};

/// How long a request may take before it is abandoned.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Holds the connection pool
pub struct TcpTransport {
    client: reqwest::Client,
}

impl TcpTransport {
    /// Builds the client.
    pub fn new() -> Result<Self, ChatError> {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(Policy::none())
            .build()
            .map_err(|error| ChatError::Config(error.to_string()))?;

        Ok(Self { client })
    }
}

#[async_trait]
impl Transport for TcpTransport {
    async fn request(
        &self,
        request: http::Request<Bytes>,
    ) -> Result<http::Response<Bytes>, TransportError> {
        let (parts, body) = request.into_parts();
        let mut outgoing = self
            .client
            .request(parts.method, parts.uri.to_string())
            .body(body);
        for (name, value) in &parts.headers {
            outgoing = outgoing.header(name, value);
        }

        let reply = outgoing.send().await.map_err(failure)?;
        let status = reply.status();
        let headers = reply.headers().clone();
        let body = read_capped(reply).await?;

        let mut response = http::Response::builder().status(status);
        // The builder holds a Result, so the headers go in through the map it is holding.
        if let Some(existing) = response.headers_mut() {
            *existing = headers;
        }

        response
            .body(body)
            .map_err(|error| TransportError::Protocol(error.to_string()))
    }
}

/// Reads the body, stopping at [`MAX_BODY_BYTES`] rather than believing a `content-length`.
async fn read_capped(mut reply: reqwest::Response) -> Result<Bytes, TransportError> {
    let mut body = BytesMut::new();

    while let Some(chunk) = reply.chunk().await.map_err(failure)? {
        if body.len() + chunk.len() > MAX_BODY_BYTES {
            return Err(TransportError::BodyTooLarge {
                limit: MAX_BODY_BYTES,
            });
        }
        body.extend_from_slice(&chunk);
    }

    Ok(body.freeze())
}

/// Sorts a reqwest failure into the taxonomy every transport reports.
///
/// Ordered from the most specific test to the least, because reqwest's categories overlap: a
/// timeout while connecting answers yes to both `is_timeout` and `is_connect`.
fn failure(error: reqwest::Error) -> TransportError {
    let detail = describe(&error);

    if error.is_timeout() {
        TransportError::Timeout
    } else if error.is_connect() {
        TransportError::Connect(detail)
    } else if error.is_request() || error.is_builder() {
        TransportError::Protocol(detail)
    } else if error.is_body() || error.is_decode() {
        TransportError::StreamReset(detail)
    } else {
        TransportError::Connect(detail)
    }
}

/// The failure and its causes, outermost first. The outermost alone names the request, not what
/// went wrong with it.
fn describe(error: &reqwest::Error) -> String {
    let mut described = error.to_string();
    let mut cause = error.source();

    while let Some(layer) = cause {
        described.push_str(": ");
        described.push_str(&layer.to_string());
        cause = layer.source();
    }

    described
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A port nothing is listening on is a failure to reach the server, not a timeout: the
    /// operating system answers immediately.
    #[tokio::test]
    async fn a_closed_port_is_reported_as_a_failure_to_connect() {
        let transport = TcpTransport::new().expect("a transport");
        let request = http::Request::get("http://127.0.0.1:1/api/v1/healthz")
            .body(Bytes::new())
            .expect("a request");

        let error = transport.request(request).await.expect_err("no listener");

        let TransportError::Connect(detail) = &error else {
            panic!("expected a connect failure, got {error:?}");
        };
        // Every platform words this differently, and every one of them says "refused".
        assert!(
            detail.contains("refused"),
            "the reason has to survive, not just the url: {detail}",
        );
    }

    /// A name that cannot resolve fails before anything is dialled. reqwest reports resolution
    /// under `is_connect`, so this lands there rather than in its own variant.
    #[tokio::test]
    async fn a_name_that_does_not_resolve_fails_without_reaching_the_wire() {
        let transport = TcpTransport::new().expect("a transport");
        let request =
            http::Request::get("http://a.host.that.does.not.exist.invalid/api/v1/healthz")
                .body(Bytes::new())
                .expect("a request");

        let error = transport.request(request).await.expect_err("no such host");

        assert!(
            matches!(
                error,
                TransportError::Connect(_) | TransportError::Resolution(_)
            ),
            "expected the dial to fail, got {error:?}",
        );
    }
}
