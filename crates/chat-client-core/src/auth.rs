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
//! Trading an API key for the tokens the SNAP asks for.

use anapaya_aa_client::{ApiKeyTokenRefresher, CrpcAaAuthClient};
use scion_http3::scion_stack::reqwest_connect_rpc::token_source::refresh::RefreshTokenSource;

use crate::{config::ApiKeyAuth, error::ChatError};

/// A source of tokens, which keeps minting them for as long as it lives.
///
/// The first exchange happens here, so a key the authority rejects fails on the connect screen
/// rather than as a connection that never comes up.
pub(crate) async fn token_source(auth: &ApiKeyAuth) -> Result<RefreshTokenSource, ChatError> {
    let client = CrpcAaAuthClient::new(&auth.aa_url).map_err(|error| {
        ChatError::Config(format!(
            "the authority at {} could not be reached: {error}",
            auth.aa_url
        ))
    })?;

    let refresher =
        ApiKeyTokenRefresher::new(client, auth.key.as_str().to_owned(), auth.device_id.clone());
    let (first, _metadata) = refresher.refresh_with_metadata().await.map_err(|error| {
        // A key the authority refuses is worth separating from an authority that is down: one is
        // the user's to fix and the other is not.
        if error.is_transient() {
            ChatError::Config(format!(
                "the authority at {} is not answering: {error}",
                auth.aa_url
            ))
        } else {
            ChatError::Config(format!("the authority refused this API key: {error}"))
        }
    })?;

    Ok(RefreshTokenSource::builder("aa", refresher)
        .with_initial_token(first)
        .build())
}
