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
//! What to start, and where to let it be reached.

use std::{net::IpAddr, path::PathBuf};

use clap::Parser;

/// A SCION network on this machine, with the chat server in it.
#[derive(Debug, Clone, Parser)]
#[command(version, about)]
pub struct Config {
    /// Where the description of this network is served.
    ///
    /// The one address fixed in advance, and therefore the only thing a client has to be told:
    /// everything else is decided at startup and read from here.
    #[arg(long, env = "CHAT_DEV_CONTROL_PORT", default_value_t = 8099)]
    pub control_port: u16,

    /// The address every part of this network listens on.
    ///
    /// Drives the topology, the control API and the server together. Splitting them would leave a
    /// loopback socket unable to reach a tunnel that is not on loopback.
    ///
    /// Never a wildcard: the SNAP tunnel is dialled at this address, and `0.0.0.0` names no host.
    /// Give this machine's own address to be reached from another one.
    #[arg(long, env = "CHAT_DEV_BIND_IP", default_value = "127.0.0.1")]
    pub bind_ip: IpAddr,

    /// The address to tell clients this network is at, when it differs from where it listens.
    ///
    /// For a client that reaches this host by another route: an Android emulator reaches the
    /// host's loopback as `10.0.2.2`, so the bind address stays where it is and only what is
    /// published moves. Applies to the AS a client attaches to, not the one the server sits
    /// in.
    #[arg(long, env = "CHAT_DEV_ADVERTISE_IP")]
    pub advertise_ip: Option<IpAddr>,

    /// The port the chat server listens on.
    ///
    /// Fixed rather than free, so the URL a client is given is the same between runs.
    #[arg(long, env = "CHAT_DEV_SERVER_PORT", default_value_t = 8443)]
    pub server_port: u16,

    /// Where the certificate and the database go.
    ///
    /// A fresh directory each run unless one is named, so a run starts with no accounts. A server
    /// started separately has to be given the same one.
    #[arg(long, env = "CHAT_DEV_DATA_DIR")]
    pub data_dir: Option<PathBuf>,

    /// Hold the network up, and leave the chat server to be started alongside.
    #[arg(long, env = "CHAT_DEV_NO_SERVER")]
    pub no_server: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_command_line_is_well_formed() {
        <Config as clap::CommandFactory>::command().debug_assert();
    }

    /// What someone gets by running it with nothing.
    #[test]
    fn the_defaults_start_a_whole_network_on_loopback() {
        let config = Config::parse_from(["chat-dev"]);

        assert_eq!(config.control_port, 8099);
        assert_eq!(config.bind_ip, IpAddr::from([127, 0, 0, 1]));
        assert_eq!(config.server_port, 8443);
        assert_eq!(config.advertise_ip, None);
        assert_eq!(config.data_dir, None);
        assert!(!config.no_server);
    }

    /// The shape an emulator needs: bound where it already is, published where the emulator looks.
    #[test]
    fn what_is_published_moves_without_the_bind() {
        let config = Config::parse_from(["chat-dev", "--advertise-ip", "10.0.2.2"]);

        assert_eq!(config.bind_ip, IpAddr::from([127, 0, 0, 1]));
        assert_eq!(config.advertise_ip, Some(IpAddr::from([10, 0, 2, 2])));
    }
}
