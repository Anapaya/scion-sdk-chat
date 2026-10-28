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
//! The network info: what a client needs to reach the chat server on this network.
//!
//! The network chooses most values at startup. The endhost APIs take free ports, and the
//! certificate is new. The control port is the one fixed value, so a client needs only that port
//! to read the rest from `GET /info`.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// What a client needs to reach the chat server on this network.
///
/// `chat-dev` prints it as one line of JSON on standard output, and serves it at `GET /info`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevNetwork {
    /// The URL that serves this network info.
    pub control_url: String,
    /// Whether this process runs the chat server, or you start it.
    pub server: Server,
    /// The endhost API that a client uses.
    pub endhost_api_url: String,
    /// The endhost API of the server's AS, for a chat server that you start yourself.
    pub server_endhost_api_url: String,
    /// A token for the endhost API and the SNAP. Each read of `GET /info` makes a new one.
    ///
    /// Give each client its own token. The SNAP keeps one tunnel per `pssid`, so a second client
    /// with the same token evicts the first, and the first stops without an error.
    pub auth_token: String,
    /// A file with the server's token, for `chat-server --auth-token-file`.
    pub auth_token_file: String,
    /// The URL of the chat server. Its host is the name on the server's certificate.
    pub base_url: String,
    /// The server's SCION address, for `--transport scion`. This network has no TSAR records, so a
    /// client cannot resolve the host of `base_url` without it.
    pub target: String,
    /// The server's certificate in PEM, for a client that cannot read `ca_path`.
    pub ca_pem: String,
    /// The path of the server's certificate.
    pub ca_path: String,
    /// The directory with the certificate and the database. Give it to a chat server that you
    /// start yourself, so that it presents the same certificate.
    pub data_dir: String,
    /// The arguments that connect a chat server that you start yourself to this network.
    pub chat_server_args: Vec<String>,
}

/// Who starts the chat server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Server {
    /// This process starts the chat server, and stops it on exit.
    Embedded,
    /// You start the chat server, with [`DevNetwork::chat_server_args`].
    External,
}

/// The arguments that connect a chat server to this network.
pub fn chat_server_args(
    listen: &str,
    data_dir: &Path,
    endhost_api: &str,
    auth_token_file: &Path,
) -> Vec<String> {
    [
        "--transport",
        "scion",
        "--listen",
        listen,
        "--data-dir",
        &data_dir.display().to_string(),
        "--endhost-api",
        endhost_api,
        "--auth-token-file",
        &auth_token_file.display().to_string(),
    ]
    .map(str::to_owned)
    .to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn network() -> DevNetwork {
        DevNetwork {
            control_url: "http://127.0.0.1:8099".to_owned(),
            server: Server::Embedded,
            endhost_api_url: "http://127.0.0.1:41234/".to_owned(),
            server_endhost_api_url: "http://127.0.0.1:41235/".to_owned(),
            auth_token: "a token".to_owned(),
            auth_token_file: "/tmp/dev/snap.token".to_owned(),
            base_url: "https://localhost:8443".to_owned(),
            target: "2-ff00:0:212,127.0.0.1".to_owned(),
            ca_pem: "-----BEGIN CERTIFICATE-----\n".to_owned(),
            ca_path: "/tmp/dev/cert.pem".to_owned(),
            data_dir: "/tmp/dev".to_owned(),
            chat_server_args: vec!["--transport".to_owned(), "scion".to_owned()],
        }
    }

    /// The names every client parses, written out: no compiler checks them across languages.
    #[test]
    fn the_field_names_are_the_ones_clients_read() {
        let json = serde_json::to_value(network()).expect("serialize");
        let mut named: Vec<&str> = json
            .as_object()
            .expect("an object")
            .keys()
            .map(String::as_str)
            .collect();
        named.sort_unstable();

        assert_eq!(
            named,
            [
                "auth_token",
                "auth_token_file",
                "base_url",
                "ca_path",
                "ca_pem",
                "chat_server_args",
                "control_url",
                "data_dir",
                "endhost_api_url",
                "server",
                "server_endhost_api_url",
                "target",
            ]
        );
    }

    #[test]
    fn the_network_info_survives_the_round_trip_a_client_makes() {
        let network = network();

        let json = serde_json::to_string(&network).expect("serialize");
        let read: DevNetwork = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(read, network);
    }

    #[test]
    fn whether_the_server_is_here_is_written_in_snake_case() {
        let embedded = serde_json::to_string(&Server::Embedded).expect("serialize");
        let external = serde_json::to_string(&Server::External).expect("serialize");

        assert_eq!(embedded, r#""embedded""#);
        assert_eq!(external, r#""external""#);
    }

    /// You paste these into a second terminal, so they must be the real flag names.
    #[test]
    fn the_server_arguments_name_the_flags_the_server_takes() {
        let args = chat_server_args(
            "127.0.0.1:8443",
            Path::new("/tmp/dev"),
            "http://127.0.0.1:41235/",
            Path::new("/tmp/dev/snap.token"),
        );

        assert_eq!(
            args,
            [
                "--transport",
                "scion",
                "--listen",
                "127.0.0.1:8443",
                "--data-dir",
                "/tmp/dev",
                "--endhost-api",
                "http://127.0.0.1:41235/",
                "--auth-token-file",
                "/tmp/dev/snap.token",
            ]
        );
    }
}
