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
//! Every part of a chat client except the user interface.
//!
//! A user interface talks to this crate and never to a transport. Which transport is behind it is
//! configuration, so the choice of interface framework and the choice of transport are independent.
//!
//! ```text
//! any user interface
//!         │
//!         ▼
//!   this crate          the API, the session, the room feed
//!         │  Transport
//!    ┌────┴─────┬──────────────┐
//!    ▼          ▼              ▼
//! MockTransport TcpTransport   HTTP/3 over SCION
//! (no network)  (dev mode)     (the product)
//! ```
//!
//! Every wire type comes from `chat-core` and is re-exported here, so a caller needs one dependency
//! rather than two.

pub mod client;
pub mod config;
pub mod error;
pub mod transport;

pub use chat_core::api::v1;
pub use client::{ChatClient, SessionInfo};
pub use config::{ClientConfig, PollConfig, Since, TransportKind};
pub use error::{ChatError, TransportError};
pub use transport::{Transport, mock::MockTransport, tcp::TcpTransport};
