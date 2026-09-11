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
//! Three autonomous systems: one for the server, one for every client, and one for an Android
//! emulator.
//!
//! The network publishes one address for each AS. [`DEFAULT`] publishes the bound address, which
//! every client can reach. An Android emulator on this machine is the one client that cannot: it
//! reaches the host at `10.0.2.2`, and `127.0.0.1` means the emulator itself. [`EMULATOR`] holds
//! that address, so the emulator has an AS to fall back to.
//!
//! ```text
//!   1-ff00:0:132  (every client, at the bound address)
//!         |
//!   2-ff00:0:212  (the chat server)
//!         |
//!   2-ff00:0:222  (an Android emulator on this machine, at 10.0.2.2)
//! ```

use std::collections::BTreeMap;

use chrono::Utc;
/// The three autonomous systems. [`DEFAULT`] serves every client that reaches the bound
/// address.
pub use pocketscion::util::topologies::{IA132 as DEFAULT, IA212 as SERVER, IA222 as EMULATOR};
use pocketscion::{
    io_config::IoConfig,
    network::scion::topology::{ScionAs, ScionLink, ScionLinkType, ScionTopologyBuilder},
    runtime::builder::PocketScionRuntimeBuilder,
    state::PocketScionState,
    util::topologies::PsSetup,
};

/// Starts the network. Each AS gets an endhost API and a SNAP endpoint.
pub async fn start(io_config: IoConfig) -> PsSetup {
    let mut state = PocketScionState::new(Utc::now());
    state.set_topology(definition().build().expect("a well-formed topology"));

    let endhost_apis = BTreeMap::from([
        (DEFAULT, state.add_endhost_api(vec![DEFAULT])),
        (SERVER, state.add_endhost_api(vec![SERVER])),
        (EMULATOR, state.add_endhost_api(vec![EMULATOR])),
    ]);

    for isd_as in [DEFAULT, SERVER, EMULATOR] {
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

/// The three ASes, and one link from each client AS to the server's AS.
fn definition() -> ScionTopologyBuilder {
    let mut topology = ScionTopologyBuilder::new();
    topology
        .add_as(ScionAs::new_core(SERVER))
        .expect("the server's AS")
        .add_as(ScionAs::new_core(DEFAULT))
        .expect("the default AS")
        .add_as(ScionAs::new_core(EMULATOR))
        .expect("the emulator's AS")
        .add_link(
            ScionLink::new(DEFAULT, 1, ScionLinkType::Core, SERVER, 3).expect("a well-formed link"),
        )
        .expect("the default link")
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

    /// Each client AS must reach the server's AS.
    #[test]
    fn both_client_ases_are_linked_to_the_server() {
        let topology = definition().build().expect("a well-formed topology");

        for (isd_as, interface) in [(DEFAULT, 1), (EMULATOR, 2)] {
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
