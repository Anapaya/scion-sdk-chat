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
//! The connection form, answered before the terminal is taken.
//!
//! Every flag is one field of it, and none is required: what is not given is left for the first
//! screen to ask for.
//!
//! The variables are `CHAT_CLIENT_*` rather than `CHAT_*` because `chat-server` already reads
//! `CHAT_ENDHOST_API`. The server sits in one AS and this client attaches to another, so a shared
//! variable would point the client at the wrong endhost API.

use clap::Parser;

use crate::screens::connection::Settings;

/// A terminal chat client, over TCP or over SCION.
#[derive(Debug, Clone, Parser)]
#[command(version, about)]
pub struct Config {
    /// Where the server is. The scheme picks the transport: `http` plain, `https` over SCION.
    #[arg(long, env = "CHAT_CLIENT_SERVER_URL")]
    server_url: Option<String>,

    /// The endhost API to find the SCION network through. Required by an `https` URL.
    ///
    /// The client's own AS, not the server's. A local `chat-dev` prints both.
    #[arg(long, env = "CHAT_CLIENT_ENDHOST_API")]
    endhost_api: Option<String>,

    /// The server's SCION address, without a port, for a host with no TSAR record.
    #[arg(long, env = "CHAT_CLIENT_TARGET")]
    target: Option<String>,

    /// A certificate to trust instead of the system roots.
    #[arg(long, env = "CHAT_CLIENT_CERT_PATH")]
    cert_path: Option<String>,

    /// The token the SNAP underlay asks for.
    ///
    /// Better given as `CHAT_CLIENT_SNAP_TOKEN`: an argument is readable by anyone who can list
    /// processes.
    #[arg(long, env = "CHAT_CLIENT_SNAP_TOKEN")]
    snap_token: Option<String>,
}

impl Config {
    /// The form as the first screen shows it: what was given, and the defaults for the rest.
    pub fn settings(self) -> Settings {
        let blank = Settings::default();

        Settings {
            server_url: self.server_url.unwrap_or(blank.server_url),
            endhost_api: self.endhost_api.unwrap_or(blank.endhost_api),
            target: self.target.unwrap_or(blank.target),
            cert_path: self.cert_path.unwrap_or(blank.cert_path),
            snap_token: self.snap_token.unwrap_or(blank.snap_token),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_line_is_well_formed() {
        <Config as clap::CommandFactory>::command().debug_assert();
    }

    /// A launch with no arguments is still the development launch it always was.
    #[test]
    fn nothing_given_leaves_the_defaults() {
        let settings = Config::parse_from(["chat-ui-ratatui"]).settings();

        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn every_field_of_the_form_has_a_flag() {
        let settings = Config::parse_from([
            "chat-ui-ratatui",
            "--server-url",
            "https://localhost:8443",
            "--endhost-api",
            "http://127.0.0.1:41234/",
            "--target",
            "2-ff00:0:212,127.0.0.1",
            "--cert-path",
            "/tmp/dev/cert.pem",
            "--snap-token",
            "a token",
        ])
        .settings();

        assert_eq!(
            settings,
            Settings {
                server_url: "https://localhost:8443".to_owned(),
                endhost_api: "http://127.0.0.1:41234/".to_owned(),
                target: "2-ff00:0:212,127.0.0.1".to_owned(),
                cert_path: "/tmp/dev/cert.pem".to_owned(),
                snap_token: "a token".to_owned(),
            }
        );
    }
}
