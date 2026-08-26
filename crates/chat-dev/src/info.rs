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
//! What this network is, in the words a client needs to reach it.
//!
//! Almost none of it can be written down ahead of time: the endhost APIs take whatever ports are
//! free, the token is minted per run, and the certificate is generated. Only the control port is
//! fixed, which is what makes it the one thing a client has to be told.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Where the chat server is, for a client that wants to reach it.
///
/// Printed as one line of JSON on standard output at startup, and served at `GET /info`. Both,
/// because a harness that started this process can read the line and one that did not cannot —
/// an emulator or a container has neither the terminal nor the filesystem.
///
/// `Deserialize` as well as `Serialize` so a test can read it back into this type, which is what
/// keeps the field names from drifting away from the clients that parse them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevNetwork {
    /// Where this description is served, for anything that cannot read standard output.
    pub control_url: String,
    /// What carries SCION traffic between the two ASes.
    pub underlay: String,
    /// Whether the chat server runs in this process, or is expected alongside it.
    pub server: Server,
    /// The endhost API of the AS a client attaches to, which is what a client is configured with.
    pub endhost_api_url: String,
    /// The endhost API of the AS the server sits in, for a server started separately.
    pub server_endhost_api_url: String,
    /// The AS a client attaches to.
    pub client_isd_as: String,
    /// A token for the endhost API and the SNAP control plane, minted for whoever read this.
    ///
    /// One per client, never shared. A token carries a `pssid`, and the SNAP control plane keeps
    /// one tunnel identity per `pssid`: a second client registering the same one evicts the first,
    /// whose packets then reach a gateway that no longer holds its keys. Two clients sharing a
    /// token do not fail to start — the one that started first stops working.
    pub auth_token: String,
    /// The server's own token, on disk, for `chat-server --auth-token-file`.
    ///
    /// Deliberately not the token above: the server is a client of the network too, and needs a
    /// `pssid` of its own for the same reason.
    pub auth_token_file: String,
    /// Where the chat server is, as a URL. The host is the name its certificate is issued for.
    pub base_url: String,
    /// The server's SCION address, without a port. This topology has no TSAR records, so a client
    /// is given the address rather than resolving the name.
    pub target: String,
    /// The certificate the server presents, to be trusted as an anchor.
    ///
    /// Inline as well as on disk, because a client on an emulator cannot read this filesystem.
    pub ca_pem: String,
    /// The same certificate on disk, for a client that takes a path.
    pub ca_path: String,
    /// Its SHA-256, as the server logs it.
    pub ca_fingerprint: String,
    /// Where the certificate and the database live. A server started separately must be given the
    /// same one, or it will present a certificate this description does not describe.
    pub data_dir: String,
    /// The arguments that run a server against this network, for `--no-server`.
    pub chat_server_args: Vec<String>,
}

/// Whether the chat server runs here or alongside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Server {
    /// Started by this process, and stopped with it.
    Embedded,
    /// Left to the reader to start, with [`DevNetwork::chat_server_args`].
    External,
}

/// The arguments a server needs to join this network.
///
/// Built here rather than written in a README so that the ports, which change every run, cannot be
/// stale by the time anyone reads them.
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

    /// The names every client parses. A rename here breaks a client written in another language,
    /// where nothing would catch it, so the set is written out rather than derived.
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

    /// What a reader pastes into a second terminal, so it has to be the real flag names.
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
