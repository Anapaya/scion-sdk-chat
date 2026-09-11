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
//! The description of this network, in the values a client needs to reach it.
//!
//! The network decides most of these values at startup. The endhost APIs take free ports. The
//! network makes a token for each run, and it generates the certificate. The control port is the
//! one fixed value, so a client needs only that port to find the rest.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Where the chat server is, for a client that must reach it.
///
/// The process prints this as one line of JSON on standard output, and serves it at `GET /info`.
/// A client in an emulator or a container reads the served copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevNetwork {
    /// Where this description is served, for a client that reads no standard output.
    pub control_url: String,
    /// What carries SCION traffic between the two ASes.
    pub underlay: String,
    /// Whether the chat server runs in this process, or is expected alongside it.
    pub server: Server,
    /// The endhost API of the AS a client attaches to. A client uses this one.
    pub endhost_api_url: String,
    /// The endhost API of the AS the server sits in, for a server started separately.
    pub server_endhost_api_url: String,
    /// The AS a client attaches to.
    pub client_isd_as: String,
    /// A token for the endhost API and the SNAP control plane. Each read makes a new one.
    ///
    /// Give each client its own token. The control plane keeps one tunnel for each `pssid`. A
    /// second client on the same token removes the tunnel of the first client, and the first
    /// client then stops without an error.
    pub auth_token: String,
    /// The server's own token, on disk, for `chat-server --auth-token-file`.
    pub auth_token_file: String,
    /// Where the chat server is, as a URL. The host is the name its certificate is issued for.
    pub base_url: String,
    /// The server's SCION address, without a port. This topology holds no TSAR records, so the
    /// description gives a client the address to dial.
    pub target: String,
    /// The certificate the server presents. A client trusts it as an anchor.
    ///
    /// The description holds the text as well as the path, for a client that reads a different
    /// filesystem.
    pub ca_pem: String,
    /// The same certificate on disk, for a client that takes a path.
    pub ca_path: String,
    /// Its SHA-256, as the server logs it.
    pub ca_fingerprint: String,
    /// Where the certificate and the database live. Give the same directory to a chat server that
    /// you start yourself, so it presents the certificate this description names.
    pub data_dir: String,
    /// The arguments that join a chat server to this network, for `--no-server`.
    pub chat_server_args: Vec<String>,
}

/// Whether the chat server runs here or alongside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Server {
    /// Started by this process, and stopped with it.
    Embedded,
    /// You start it, with [`DevNetwork::chat_server_args`].
    External,
}

/// The arguments a chat server needs to join this network.
///
/// The ports change for each run, so this function builds the arguments from the current values.
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
            underlay: "snap".to_owned(),
            server: Server::Embedded,
            endhost_api_url: "http://127.0.0.1:41234/".to_owned(),
            server_endhost_api_url: "http://127.0.0.1:41235/".to_owned(),
            client_isd_as: "1-ff00:0:132".to_owned(),
            auth_token: "a token".to_owned(),
            auth_token_file: "/tmp/dev/snap.token".to_owned(),
            base_url: "https://localhost:8443".to_owned(),
            target: "2-ff00:0:212,127.0.0.1".to_owned(),
            ca_pem: "-----BEGIN CERTIFICATE-----\n".to_owned(),
            ca_path: "/tmp/dev/cert.pem".to_owned(),
            ca_fingerprint: "ab12".to_owned(),
            data_dir: "/tmp/dev".to_owned(),
            chat_server_args: vec!["--transport".to_owned(), "scion".to_owned()],
        }
    }

    /// The names every client parses. This test writes them out in full, because a client in
    /// another language reads the same names and no compiler checks them.
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
                "ca_fingerprint",
                "ca_path",
                "ca_pem",
                "chat_server_args",
                "client_isd_as",
                "control_url",
                "data_dir",
                "endhost_api_url",
                "server",
                "server_endhost_api_url",
                "target",
                "underlay",
            ]
        );
    }

    #[test]
    fn a_description_survives_the_round_trip_a_client_makes() {
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
