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

/// The address [`ClientConfig::default`] leaves behind, and the server's own dev-mode address.
const DEV_SERVER_URL: &str = "http://localhost:8080";

/// Which transport to build
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    /// HTTP/3 over SCION.
    Scion(ScionConfig),
    /// Plain HTTP over TCP, against the server's development mode.
    #[default]
    Tcp,
}

/// SCION transport configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScionConfig {
    /// The endhost API to reach the SCION network through.
    pub endhost_api: Url,
    /// A token, needed only on the SNAP underlay.
    pub snap_token: Option<SnapToken>,
    /// The SCION address to dial, for a host with no TSAR record. Portless: the port always comes
    /// from `server_url`.
    pub target: Option<String>,
    /// Which certificates the client accepts from the server.
    #[serde(default)]
    pub trust: Trust,
}

/// Which certificates a client accepts from the server.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Trust {
    /// The anchors the operating system ships.
    #[default]
    SystemRoots,
    /// One certificate, in place of the system roots. A self-signed server needs this.
    Pinned(PathBuf),
    /// Accept any certificate.
    ///
    /// Every reply can come from anyone on the path. It exists so a demo can run against a
    /// self-signed server without moving its certificate first.
    Insecure,
}

/// A token for the SNAP underlay.
///
/// `Debug` prints a placeholder. Serialization is not redacted.
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

impl std::str::FromStr for SnapToken {
    type Err = std::convert::Infallible;

    /// Any string is a token here. The server decides whether it is a good one.
    fn from_str(token: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(token))
    }
}

/// Everything a client reads at startup. Plain data, so the whole value serializes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    /// Which transport to talk over.
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
    /// How long to wait between reads of the open room's messages.
    ///
    /// Both intervals sit under the SDK's idle connection timeout, so polling rides one
    /// connection.
    pub messages_interval: Duration,
    /// How long to wait between reads of the room list. Slower: a late room costs nothing.
    pub rooms_interval: Duration,
    /// How many messages to ask for at a time.
    pub page_limit: usize,
}

impl PollConfig {
    /// The page size to ask for, never zero: a zero page counts as full and never stops.
    pub fn page_size(&self) -> usize {
        self.page_limit.max(1)
    }
}

impl Default for PollConfig {
    fn default() -> Self {
        Self {
            messages_interval: Duration::from_secs(2),
            rooms_interval: Duration::from_secs(2),
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

        assert_eq!(poll.messages_interval, Duration::from_secs(2));
        assert_eq!(poll.rooms_interval, Duration::from_secs(2));
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
                trust: Trust::Pinned(PathBuf::from("chat-server.pem")),
            }),
            server_url: Url::parse("http://127.0.0.1:8080").expect("a url"),
            poll: PollConfig {
                messages_interval: Duration::from_millis(500),
                rooms_interval: Duration::from_secs(5),
                page_limit: 10,
            },
        };

        let json = serde_json::to_string(&config).expect("serialize");
        let decoded: ClientConfig = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(decoded.transport, config.transport);
        assert_eq!(decoded.server_url, config.server_url);
        assert_eq!(decoded.poll, config.poll);
    }

    /// A config reaches a log through `Debug`, and the token must not go with it.
    #[test]
    fn the_snap_token_is_redacted_in_debug_output() {
        let config = ClientConfig {
            transport: TransportKind::Scion(ScionConfig {
                endhost_api: Url::parse("http://127.0.0.1:8041").expect("a url"),
                snap_token: Some(SnapToken::new("s3cret")),
                target: None,
                trust: Trust::SystemRoots,
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

    /// The names a settings file is written in.
    #[test]
    fn the_transport_is_named_in_snake_case_on_the_wire() {
        let tcp = serde_json::to_string(&TransportKind::Tcp).expect("serialize");
        let scion = serde_json::to_string(&TransportKind::Scion(ScionConfig {
            endhost_api: Url::parse("http://127.0.0.1:8041").expect("a url"),
            snap_token: None,
            target: None,
            trust: Trust::SystemRoots,
        }))
        .expect("serialize");

        assert_eq!(tcp, r#""tcp""#);
        assert!(
            scion.starts_with(r#"{"scion":"#),
            "the settings go under the transport's name: {scion}"
        );
    }

    /// A settings file written before [`Trust`] existed names no trust, and must still load.
    #[test]
    fn a_config_without_a_trust_reads_as_the_system_roots() {
        let json = r#"{
            "transport": {"scion": {
                "endhost_api": "http://127.0.0.1:8041/",
                "snap_token": null,
                "target": null
            }},
            "server_url": "https://localhost:8443/",
            "poll": {"messages_interval": {"secs": 2, "nanos": 0},
                     "rooms_interval": {"secs": 2, "nanos": 0},
                     "page_limit": 50}
        }"#;

        let config: ClientConfig = serde_json::from_str(json).expect("deserialize");

        let TransportKind::Scion(scion) = config.transport else {
            panic!("a scion transport");
        };
        assert_eq!(scion.trust, Trust::SystemRoots);
    }

    /// Each choice survives a settings file, the pinned path included.
    #[test]
    fn every_trust_survives_a_round_trip() {
        for trust in [
            Trust::SystemRoots,
            Trust::Pinned(PathBuf::from("chat-server.pem")),
            Trust::Insecure,
        ] {
            let json = serde_json::to_string(&trust).expect("serialize");
            let decoded: Trust = serde_json::from_str(&json).expect("deserialize");

            assert_eq!(decoded, trust, "{json}");
        }
    }

    /// The default is the strict one: a missing choice never means a missing check.
    #[test]
    fn the_default_trust_verifies() {
        assert_eq!(Trust::default(), Trust::SystemRoots);
    }
}
