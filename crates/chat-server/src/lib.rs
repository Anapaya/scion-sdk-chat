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
//! The chat server runtime, exposed as a library so that tests can embed the server in-process.

use crate::{
    api::AppState,
    auth::Tokens,
    config::{Config, Transport},
    store::SqliteStore,
};

pub mod api;
pub mod auth;
pub mod config;
pub mod store;

/// Anything that stops the server from starting.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    /// The database could not be opened.
    #[error(transparent)]
    Store(#[from] store::StoreError),
    /// The token-signing secret could not be read or written.
    #[error(transparent)]
    Auth(#[from] auth::AuthError),
    /// The listener could not be bound, or serving stopped with an error.
    #[error("serving on {addr}: {source}")]
    Serve {
        /// The address being bound.
        addr: std::net::SocketAddr,
        /// What the operating system reported.
        source: std::io::Error,
    },
    /// The transport asked for is not implemented yet.
    #[error("{0}")]
    Unsupported(&'static str),
}

/// Opens the store, prepares the auth material, and serves the API until the process is asked to
/// stop.
pub async fn run(config: Config) -> Result<(), RunError> {
    let state = state(&config).await?;
    let router = api::router(state);

    match config.transport {
        Transport::Tcp => serve_tcp(&config, router).await,
        Transport::Scion => {
            Err(RunError::Unsupported(
                "--transport scion is not implemented yet; use --transport tcp",
            ))
        }
    }
}

/// Everything the handlers need, built from the configuration.
pub async fn state(config: &Config) -> Result<AppState, RunError> {
    let store = SqliteStore::new(&config.database(), config.caps()).await?;
    // The store creates the data directory, so the secret can be written beside the database
    // without checking for it again.
    let secret = auth::load_or_create_secret(&config.jwt_secret())?;

    Ok(AppState {
        store: Box::new(store),
        tokens: Tokens::new(&secret, config.token_validity()),
        config: config.clone(),
    })
}

/// Serves over plain TCP. Development only: no TLS, and every endpoint is curl-able.
async fn serve_tcp(config: &Config, router: axum::Router) -> Result<(), RunError> {
    let addr = config.listen;
    let fail = |source| RunError::Serve { addr, source };

    let listener = tokio::net::TcpListener::bind(addr).await.map_err(fail)?;
    tracing::info!(%addr, data_dir = %config.data_dir.display(), "serving over tcp");

    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("shutting down");
        })
        .await
        .map_err(fail)
}
