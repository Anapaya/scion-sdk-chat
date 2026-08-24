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
//! HTTP/3 over SCION. The stack, the paths and the handshake are all [`scion_http3::Client`]'s.

use std::{str::FromStr as _, sync::Arc};

use async_trait::async_trait;
use bytes::Bytes;
use scion_http3::{
    Client, Config, Error, Request, scion_quic::quic::config::QuicConfig,
    scion_stack::resolver::txt::ScionTxtDnsResolver, sciparse::address::ip_addr::ScionIpAddr,
};
use url::Url;

use super::{MAX_BODY_BYTES, Transport};
use crate::{
    config::ScionConfig,
    error::{ChatError, TransportError},
};

/// Holds the connection pool, and the stack underneath it.
pub struct ScionTransport {
    client: Client,
}

impl ScionTransport {
    /// Builds the client. Nothing is dialled: the stack is built on the first request.
    ///
    /// `server_url` is read for its host, which is the name a `target` answers for.
    pub fn new(config: &ScionConfig, server_url: &Url) -> Result<Self, ChatError> {
        // Reaching the endhost API goes through rustls, which installs no default provider while
        // both backends are in the build. Idempotent, and the TCP transport does the same.
        scion_sdk_utils::rustls::select_ring_crypto_provider();

        let mut settings = Config::new(config.endhost_api.clone());

        if let Some(token) = &config.snap_token {
            settings = settings.with_auth_token(token.as_str());
        }

        // A pinned certificate replaces the system roots; the server signs its own.
        if let Some(path) = &config.cert_path {
            let path = path.to_str().ok_or_else(|| {
                ChatError::Config(format!("the certificate path is not utf-8: {path:?}"))
            })?;

            settings = settings.with_quic_config(QuicConfig::builder().ca_certs_file(path).build());
        }

        if let Some(target) = &config.target {
            settings = settings.with_resolver(Arc::new(resolver(server_url, target)?));
        }

        Ok(Self {
            client: Client::new(settings),
        })
    }
}

#[async_trait]
impl Transport for ScionTransport {
    async fn request(
        &self,
        request: http::Request<Bytes>,
    ) -> Result<http::Response<Bytes>, TransportError> {
        let (parts, body) = request.into_parts();
        let mut outgoing = Request::builder()
            .method(parts.method)
            .url(parts.uri.to_string())
            .body(body);
        for (name, value) in &parts.headers {
            outgoing = outgoing.header(name, value);
        }

        let outgoing = outgoing
            .build()
            .map_err(|error| TransportError::Protocol(error.to_string()))?;
        let reply = self.client.request(outgoing).await.map_err(failure)?;

        let status = reply.status();
        let headers = reply.headers().clone();
        // The cap is enforced by the collector rather than by trusting a `content-length`.
        let (body, _trailers) = reply.bytes(Some(MAX_BODY_BYTES)).await.map_err(failure)?;

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

/// A resolver that answers for the server's host from configuration, and every other host normally.
fn resolver(server_url: &Url, target: &str) -> Result<ScionTxtDnsResolver, ChatError> {
    let host = server_url
        .host_str()
        .ok_or_else(|| ChatError::Config(format!("the server url has no host: {server_url}")))?;
    let address = ScionIpAddr::from_str(target).map_err(|error| {
        ChatError::Config(format!("the target is not a scion address: {error}"))
    })?;

    ScionTxtDnsResolver::new()
        .map(|resolver| resolver.with_override(host, vec![address]))
        .map_err(|error| ChatError::Config(format!("building a resolver failed: {error}")))
}

/// Sorts an HTTP/3 failure into the taxonomy every transport reports.
fn failure(error: Error) -> TransportError {
    let detail = error.to_string();

    match error {
        Error::Resolution { .. } => TransportError::Resolution(detail),
        Error::Connect { .. } | Error::StackBuild { .. } => TransportError::Connect(detail),
        Error::Tls { .. } => TransportError::Tls(detail),
        Error::StreamReset { .. } | Error::ConnectionLimit => TransportError::StreamReset(detail),
        Error::BodyTooLarge { limit, .. } => TransportError::BodyTooLarge { limit },
        Error::Timeout { .. } => TransportError::Timeout,
        _ => TransportError::Protocol(detail),
    }
}
