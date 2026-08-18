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
//! The trait every transport implements.

use async_trait::async_trait;
use bytes::Bytes;

use crate::error::TransportError;

pub mod mock;
pub mod tcp;

/// The largest reply any transport reads, so a broken or hostile server cannot exhaust memory.
pub const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

/// Puts a request on the wire and brings the reply back.
///
/// The request arrives fully formed — absolute URL, headers set, body encoded — so an
/// implementation never inspects, rewrites or decodes anything. That is what keeps the chat API
/// out of the transports, and it is why a mock can hand back a body no real server would send.
///
/// Free of type parameters on purpose: `Arc<dyn Transport>` is how the transport is chosen at
/// runtime.
#[async_trait]
pub trait Transport: Send + Sync + 'static {
    /// Sends `request` and returns what came back.
    async fn request(
        &self,
        request: http::Request<Bytes>,
    ) -> Result<http::Response<Bytes>, TransportError>;
}
