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
//! Three autonomous systems: the server, and one for each kind of client.
//!
//! `pocketscion` ships a two-AS topology, which is one short. An advertised IP is set per AS, so
//! two clients that reach this host by different routes need an AS each: a client on this machine
//! is told `127.0.0.1`, and one on an Android emulator is told `10.0.2.2`. With both in the same
//! AS, whichever address is published is wrong for one of them.
//!
//! ```text
//!   1-ff00:0:132 ──┐                       clients on this machine
//!                  ├── 2-ff00:0:212        the chat server
//!   2-ff00:0:222 ──┘                       clients on an Android emulator
//! ```

use std::collections::BTreeMap;

use chrono::Utc;
/// The AS a client on this machine attaches to, the AS the server sits in, and the AS a client
/// on an Android emulator attaches to.
///
/// Renamed on the way in: the rest of this crate reasons about which client an AS belongs to.
pub use pocketscion::util::topologies::{IA132 as LOCAL, IA212 as SERVER, IA222 as EMULATOR};
use pocketscion::{
    io_config::IoConfig,
    network::scion::topology::{ScionAs, ScionLink, ScionLinkType, ScionTopologyBuilder},
    runtime::builder::PocketScionRuntimeBuilder,
    state::PocketScionState,
    util::topologies::PsSetup,
};

/// Starts the network described by this module's diagram.
///
/// Every AS gets a SNAP endpoint and an endhost API, the server's included: each one reaches the
/// underlay through its own.
pub async fn start(io_config: IoConfig) -> PsSetup {
    let mut state = PocketScionState::new(Utc::now());
    state.set_topology(definition().build().expect("a well-formed topology"));

    let endhost_apis = BTreeMap::from([
        (LOCAL, state.add_endhost_api(vec![LOCAL])),
        (SERVER, state.add_endhost_api(vec![SERVER])),
        (EMULATOR, state.add_endhost_api(vec![EMULATOR])),
    ]);

    for isd_as in [LOCAL, SERVER, EMULATOR] {
        state.add_snap(isd_as).expect("a SNAP endpoint");
    }

    let runtime = PocketScionRuntimeBuilder::new()
        .with_system_state(state)
        .with_io_config(io_config)
        .start()
        .await
        .expect("the network starts");

    PsSetup {
        runtime,
        endhost_apis,
    }
}

/// The ASes and the links between them, without a runtime.
///
/// A star: each client AS is linked to the server's, and to nothing else.
fn definition() -> ScionTopologyBuilder {
    let mut topology = ScionTopologyBuilder::new();
    topology
        .add_as(ScionAs::new_core(SERVER))
        .expect("the server's AS")
        .add_as(ScionAs::new_core(LOCAL))
        .expect("the local AS")
        .add_as(ScionAs::new_core(EMULATOR))
        .expect("the emulator's AS")
        .add_link(
            ScionLink::new(LOCAL, 1, ScionLinkType::Core, SERVER, 3).expect("a well-formed link"),
        )
        .expect("the local link")
        .add_link(
            ScionLink::new(EMULATOR, 2, ScionLinkType::Core, SERVER, 4)
                .expect("a well-formed link"),
        )
        .expect("the emulator link");
    topology
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A link from each client AS to the server is the whole point of the third AS.
    #[test]
    fn both_client_ases_are_linked_to_the_server() {
        let topology = definition().build().expect("a well-formed topology");

        for (isd_as, interface) in [(LOCAL, 1), (EMULATOR, 2)] {
            let link = topology
                .scion_link(&isd_as, interface)
                .unwrap_or_else(|| panic!("{isd_as} has a link on interface {interface}"));

            assert_eq!(
                link.get_peer(&isd_as).map(|peer| peer.isd_as),
                Some(SERVER),
                "{isd_as} is linked to the server",
            );
        }
    }
}
