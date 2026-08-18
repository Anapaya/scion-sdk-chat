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
//! Configuration for a client.

use std::{fmt, path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};
use url::Url;

/// The address [`ClientConfig::default`] leaves behind, which is also the server's own dev-mode
/// address. A mock never dials it: it matches on method and path.
const DEV_SERVER_URL: &str = "http://localhost:8080";

/// Which transport to build.
///
/// A mock is absent because it is never built from configuration — a test hands one in ready-made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    /// HTTP/3 over SCION.
    #[default]
    Scion,
    /// Plain HTTP over TCP, against the server's development mode.
    Tcp,
}

/// A token for the SNAP underlay.
///
/// `Debug` prints a placeholder, so logging a config, or a panic that includes one, cannot expose
/// it. Serialization is not redacted: a settings screen that persists a config has to write it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SnapToken(String);

impl SnapToken {
    /// Wraps a token read from configuration.
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    /// The token, for the transport that sends it.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SnapToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SnapToken(<redacted>)")
    }
}

/// Everything a client reads at startup.
///
/// Plain data with no SDK types in it, so a settings screen can persist the whole value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// Which transport to talk over.
    pub transport: TransportKind,
    /// Where the chat server is, as the base every request is joined onto.
    pub server_url: Url,
    /// The endhost API to reach the SCION network through. Read by the SCION transport only.
    pub endhost_api: Option<Url>,
    /// A token, needed only on the SNAP underlay.
    pub snap_token: Option<SnapToken>,
    /// The SCION address to dial, for a host with no TSAR record. Portless: the port always comes
    /// from `server_url`.
    pub target: Option<String>,
    /// A pinned certificate to trust instead of the system roots.
    pub cert_path: Option<PathBuf>,
    /// How often to poll, and how much to ask for.
    pub poll: PollConfig,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            transport: TransportKind::default(),
            server_url: Url::parse(DEV_SERVER_URL).expect("a constant URL parses"),
            endhost_api: None,
            snap_token: None,
            target: None,
            cert_path: None,
            poll: PollConfig::default(),
        }
    }
}

/// How a watched room is kept up to date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollConfig {
    /// How long to wait between fetches. Two seconds sits under the SDK's 30-second QUIC idle
    /// timeout, so steady polling rides one warm connection.
    pub room_interval: Duration,
    /// How many messages to ask for at a time.
    pub page_limit: usize,
}

impl Default for PollConfig {
    fn default() -> Self {
        Self {
            room_interval: Duration::from_secs(2),
            page_limit: 50,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The defaults are the ones the design fixes.
    #[test]
    fn the_poll_defaults_match_the_design() {
        let poll = PollConfig::default();

        assert_eq!(poll.room_interval, Duration::from_secs(2));
        assert_eq!(poll.page_limit, 50);
    }

    /// A settings screen persists the whole value, so every field has to survive the round trip.
    #[test]
    fn a_config_survives_a_round_trip_through_json() {
        let config = ClientConfig {
            transport: TransportKind::Tcp,
            server_url: Url::parse("http://127.0.0.1:8080").expect("a url"),
            endhost_api: Some(Url::parse("http://127.0.0.1:8041").expect("a url")),
            snap_token: Some(SnapToken::new("a token")),
            target: Some("2-ff00:0:212,10.0.0.5".to_owned()),
            cert_path: Some(PathBuf::from("chat-server.pem")),
            poll: PollConfig {
                room_interval: Duration::from_millis(500),
                page_limit: 10,
            },
        };

        let json = serde_json::to_string(&config).expect("serialize");
        let decoded: ClientConfig = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.transport, config.transport);
        assert_eq!(decoded.server_url, config.server_url);
        assert_eq!(decoded.endhost_api, config.endhost_api);
        assert_eq!(decoded.snap_token, config.snap_token);
        assert_eq!(decoded.target, config.target);
        assert_eq!(decoded.cert_path, config.cert_path);
        assert_eq!(decoded.poll, config.poll);
    }

    /// A config reaches a log or a panic message through `Debug`, and the token must not go with
    /// it.
    #[test]
    fn the_snap_token_is_redacted_in_debug_output() {
        let config = ClientConfig {
            snap_token: Some(SnapToken::new("s3cret")),
            ..ClientConfig::default()
        };

        let shown = format!("{config:?}");

        assert!(!shown.contains("s3cret"), "the token is in {shown}");
        assert!(
            shown.contains("<redacted>"),
            "and its absence is visible: {shown}"
        );
    }

    /// Persisting a config has to write the real token, so serialization is not redacted.
    #[test]
    fn the_snap_token_is_written_as_a_plain_string() {
        let json = serde_json::to_string(&SnapToken::new("s3cret")).expect("serialize");

        assert_eq!(json, r#""s3cret""#);
    }

    /// The names a persisted config uses, which a settings file is written in.
    #[test]
    fn the_transport_is_named_in_snake_case_on_the_wire() {
        let json = serde_json::to_string(&TransportKind::Scion).expect("serialize");

        assert_eq!(json, r#""scion""#);
    }
}
