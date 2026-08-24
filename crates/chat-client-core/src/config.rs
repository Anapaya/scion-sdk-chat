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

/// Which transport to build, and what that transport needs.
///
/// The settings live in the variant that reads them, so a transport cannot be asked for without
/// them: there is no way to write down SCION with no endhost API, which is the one setting it
/// cannot be given a default for.
///
/// A mock is absent because it is never built from configuration — a test hands one in ready-made.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    /// HTTP/3 over SCION.
    Scion(ScionConfig),
    /// Plain HTTP over TCP, against the server's development mode.
    ///
    /// The default because it is the only one that needs nothing else to work. SCION is the
    /// transport this app is for, but no default can name it without inventing an endhost API.
    #[default]
    Tcp,
}

/// What the SCION transport needs, and what no other transport reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScionConfig {
    /// The endhost API to reach the SCION network through. Required: it is how the client finds
    /// SCION at all, and there is no address worth guessing.
    pub endhost_api: Url,
    /// A token, needed only on the SNAP underlay.
    pub snap_token: Option<SnapToken>,
    /// The SCION address to dial, for a host with no TSAR record. Portless: the port always comes
    /// from `server_url`.
    pub target: Option<String>,
    /// A pinned certificate to trust instead of the system roots.
    pub cert_path: Option<PathBuf>,
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
    /// Which transport to talk over, and what it needs.
    pub transport: TransportKind,
    /// Where the chat server is, as the base every request is joined onto.
    pub server_url: Url,
    /// How often to poll, and how much to ask for.
    pub poll: PollConfig,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            transport: TransportKind::default(),
            server_url: Url::parse(DEV_SERVER_URL).expect("a constant URL parses"),
            poll: PollConfig::default(),
        }
    }
}

/// How a watched room is kept up to date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PollConfig {
    /// How long to wait between fetches. Two seconds sits under the SDK's 25-second idle
    /// connection timeout, so steady polling rides one warm connection.
    pub room_interval: Duration,
    /// How many messages to ask for at a time.
    pub page_limit: usize,
}

impl PollConfig {
    /// The page size to ask for, never zero.
    ///
    /// A zero would make every page count as full, which reads as "more is waiting" for ever and
    /// leaves a feed fetching without pause.
    pub fn page_size(&self) -> usize {
        self.page_limit.max(1)
    }
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
            transport: TransportKind::Scion(ScionConfig {
                endhost_api: Url::parse("http://127.0.0.1:8041").expect("a url"),
                snap_token: Some(SnapToken::new("a token")),
                target: Some("2-ff00:0:212,10.0.0.5".to_owned()),
                cert_path: Some(PathBuf::from("chat-server.pem")),
            }),
            server_url: Url::parse("http://127.0.0.1:8080").expect("a url"),
            poll: PollConfig {
                room_interval: Duration::from_millis(500),
                page_limit: 10,
            },
        };

        let json = serde_json::to_string(&config).expect("serialize");
        let decoded: ClientConfig = serde_json::from_str(&json).expect("deserialize");

        // The transport carries its own settings, so comparing it compares them too.
        assert_eq!(decoded.transport, config.transport);
        assert_eq!(decoded.server_url, config.server_url);
        assert_eq!(decoded.poll, config.poll);
    }

    /// A config reaches a log or a panic message through `Debug`, and the token must not go with
    /// it.
    #[test]
    fn the_snap_token_is_redacted_in_debug_output() {
        let config = ClientConfig {
            transport: TransportKind::Scion(ScionConfig {
                endhost_api: Url::parse("http://127.0.0.1:8041").expect("a url"),
                snap_token: Some(SnapToken::new("s3cret")),
                target: None,
                cert_path: None,
            }),
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

    /// The names a persisted config uses, which a settings file is written in. A transport that
    /// carries settings writes them under its own name; one that carries none is the name alone.
    #[test]
    fn the_transport_is_named_in_snake_case_on_the_wire() {
        let tcp = serde_json::to_string(&TransportKind::Tcp).expect("serialize");
        let scion = serde_json::to_string(&TransportKind::Scion(ScionConfig {
            endhost_api: Url::parse("http://127.0.0.1:8041").expect("a url"),
            snap_token: None,
            target: None,
            cert_path: None,
        }))
        .expect("serialize");

        assert_eq!(tcp, r#""tcp""#);
        assert!(
            scion.starts_with(r#"{"scion":"#),
            "the settings go under the transport's name: {scion}"
        );
    }
}
