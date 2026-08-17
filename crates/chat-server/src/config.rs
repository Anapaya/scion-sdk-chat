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
//! Flags and their environment fallbacks.

use std::{net::SocketAddr, path::PathBuf, time::Duration};

use clap::{Parser, ValueEnum};

/// How the API is served.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Transport {
    /// HTTP/3 over SCION.
    Scion,
    /// Plain HTTP over TCP. Development only: no TLS, and curl-able.
    Tcp,
}

/// Everything the server reads at startup. Every flag has a `CHAT_` environment fallback.
#[derive(Debug, Clone, Parser)]
#[command(version, about = "A chat server over HTTP/3-over-SCION")]
pub struct Config {
    /// How to serve the API.
    #[arg(long, env = "CHAT_TRANSPORT", value_enum, default_value = "scion")]
    pub transport: Transport,

    /// Address to bind.
    #[arg(long, env = "CHAT_LISTEN", default_value = "0.0.0.0:8443")]
    pub listen: SocketAddr,

    /// Where to keep `chat.db` and `jwt.secret`. Created if absent.
    #[arg(long, env = "CHAT_DATA_DIR")]
    pub data_dir: PathBuf,

    /// How many accounts to accept.
    #[arg(long, env = "CHAT_MAX_ACCOUNTS", default_value_t = 500)]
    pub max_accounts: u32,

    /// How many rooms to accept.
    #[arg(long, env = "CHAT_MAX_ROOMS", default_value_t = 100)]
    pub max_rooms: u32,

    /// The largest message body to accept, in bytes.
    #[arg(long, env = "CHAT_MAX_MESSAGE_BYTES", default_value_t = 4096)]
    pub max_message_bytes: u32,

    /// How long a token issued at login stays valid.
    #[arg(long, env = "CHAT_TOKEN_EXPIRY_DAYS", default_value_t = 7)]
    pub token_expiry_days: u32,

    /// The endhost API to reach the SCION network through. Required by `--transport scion`.
    #[arg(long, env = "CHAT_ENDHOST_API")]
    pub endhost_api: Option<String>,

    /// A SNAP token, needed only on the SNAP underlay.
    #[arg(long, env = "CHAT_AUTH_TOKEN_FILE")]
    pub auth_token_file: Option<PathBuf>,
}

impl Config {
    /// The database file.
    pub fn database(&self) -> PathBuf {
        self.data_dir.join("chat.db")
    }

    /// The file holding the token-signing secret.
    pub fn jwt_secret(&self) -> PathBuf {
        self.data_dir.join("jwt.secret")
    }

    /// The caps the store enforces.
    pub fn caps(&self) -> crate::store::Caps {
        crate::store::Caps {
            accounts: self.max_accounts,
            rooms: self.max_rooms,
        }
    }

    /// How long an issued token stays valid.
    pub fn token_validity(&self) -> Duration {
        Duration::from_secs(u64::from(self.token_expiry_days) * 24 * 60 * 60)
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;

    use super::*;

    /// The defaults are the ones the design fixes.
    #[test]
    fn the_defaults_match_the_design() {
        let config = Config::parse_from(["chat-server", "--data-dir", "/srv/chat"]);

        assert_eq!(config.transport, Transport::Scion);
        assert_eq!(config.listen.to_string(), "0.0.0.0:8443");
        assert_eq!(config.max_accounts, 500);
        assert_eq!(config.max_rooms, 100);
        assert_eq!(config.max_message_bytes, 4096);
        assert_eq!(config.token_expiry_days, 7);
    }

    #[test]
    fn the_data_dir_places_every_file_the_server_owns() {
        let config = Config::parse_from(["chat-server", "--data-dir", "/srv/chat"]);

        assert_eq!(config.database(), PathBuf::from("/srv/chat/chat.db"));
        assert_eq!(config.jwt_secret(), PathBuf::from("/srv/chat/jwt.secret"));
    }

    #[test]
    fn the_token_lifetime_is_expressed_in_days_and_used_in_seconds() {
        let config = Config::parse_from([
            "chat-server",
            "--data-dir",
            "/srv/chat",
            "--token-expiry-days",
            "2",
        ]);

        assert_eq!(config.token_validity(), Duration::from_secs(2 * 86_400));
    }

    /// There is no default: the server never picks a directory to write into.
    #[test]
    fn the_data_directory_must_be_named() {
        assert!(Config::try_parse_from(["chat-server"]).is_err());
    }

    #[test]
    fn flags_override_the_defaults() {
        let config = Config::parse_from([
            "chat-server",
            "--data-dir",
            "/srv/chat",
            "--transport",
            "tcp",
            "--listen",
            "127.0.0.1:8080",
            "--max-accounts",
            "1",
        ]);

        assert_eq!(config.transport, Transport::Tcp);
        assert_eq!(config.listen.to_string(), "127.0.0.1:8080");
        assert_eq!(config.max_accounts, 1);
    }
}
