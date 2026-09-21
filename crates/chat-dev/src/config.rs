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
//! The flags that choose what starts, and where it listens.

use std::{net::IpAddr, path::PathBuf};

use clap::Parser;

/// A SCION network on this machine, with the chat server in it.
#[derive(Debug, Clone, Parser)]
#[command(version, about)]
pub struct Config {
    /// Where this network serves its description.
    ///
    /// Fixed. Every other address is decided at startup, and read from here.
    #[arg(long, env = "CHAT_DEV_CONTROL_PORT", default_value_t = 8099)]
    pub control_port: u16,

    /// The address every part of this network listens on.
    ///
    /// Give no wildcard: the SNAP tunnel dials this address, and `0.0.0.0` names no host. Give
    /// this machine's own address to accept a client on another machine.
    #[arg(long, env = "CHAT_DEV_BIND_IP", default_value = "127.0.0.1")]
    pub bind_ip: IpAddr,

    /// The address an Android emulator reaches this host at.
    ///
    /// Published to the emulator's AS alone.
    #[arg(long, env = "CHAT_DEV_EMULATOR_IP", default_value = "10.0.2.2")]
    pub emulator_ip: IpAddr,

    /// The port the chat server listens on. Fixed, so each run gives the same URL.
    #[arg(long, env = "CHAT_DEV_SERVER_PORT", default_value_t = 8443)]
    pub server_port: u16,

    /// Where the certificate and the database go.
    ///
    /// A new directory each run unless you name one, so a run starts with no accounts. Give the
    /// same one to a chat server you start yourself.
    #[arg(long, env = "CHAT_DEV_DATA_DIR")]
    pub data_dir: Option<PathBuf>,

    /// Start the network without the chat server, and start the server yourself.
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

    /// The values a run with no flags uses.
    #[test]
    fn the_defaults_start_a_whole_network_on_loopback() {
        let config = Config::parse_from(["chat-dev"]);

        assert_eq!(config.control_port, 8099);
        assert_eq!(config.bind_ip, IpAddr::from([127, 0, 0, 1]));
        assert_eq!(config.server_port, 8443);
        assert_eq!(config.data_dir, None);
        assert!(!config.no_server);
    }

    /// An emulator needs no flag. The default address is the one an emulator uses.
    #[test]
    fn the_emulator_is_served_by_the_defaults() {
        let config = Config::parse_from(["chat-dev"]);

        assert_eq!(config.emulator_ip, IpAddr::from([10, 0, 2, 2]));
        assert_eq!(config.bind_ip, IpAddr::from([127, 0, 0, 1]));
    }
}
