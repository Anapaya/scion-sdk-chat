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
//! Starts the network, says what it is, and holds it up.
//!
//! ```text
//! cargo run -p chat-dev
//! ```

use std::io::{IsTerminal as _, Write as _};

use chat_dev::{Config, DevSetup, Server};
use clap::Parser as _;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Standard output carries the description and nothing else, so a reader can pipe it into a
    // parser. Everything the logs have to say goes the other way.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let setup = DevSetup::start(&Config::parse()).await?;
    let network = setup.network().clone();

    println!("{}", serde_json::to_string(&network)?);
    std::io::stdout().flush()?;
    summarise(&network);

    let stop = setup.stopper();
    let serving = tokio::spawn(setup.serve());

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        // Whoever started this process closing their end is the other way to say stop, and the one
        // a harness has. Watched only when it is not a terminal, or it would eat what is typed.
        () = stdin_closed(), if !std::io::stdin().is_terminal() => {}
    }

    // Awaited rather than dropped, so the network is down before the process is.
    stop.cancel();
    serving.await?;
    Ok(())
}

/// The same description in the shape a person reads, on standard error beside the logs.
fn summarise(network: &chat_dev::DevNetwork) {
    let mut out = std::io::stderr().lock();

    let _ = writeln!(out, "\n  a SCION network is up, and it is described at");
    let _ = writeln!(out, "    {}/info\n", network.control_url);

    match network.server {
        Server::Embedded => {
            let _ = writeln!(
                out,
                "  the chat server is in this process, at {}",
                network.target
            );
            let _ = writeln!(out, "  reach it with\n");
            // The token is fetched rather than printed, so this can be pasted into as many
            // terminals as you like. Each client needs a `pssid` of its own.
            let _ = writeln!(
                out,
                "    cargo run -p chat-ui-ratatui -- \\\n      \
                 --server-url {} \\\n      \
                 --endhost-api {} \\\n      \
                 --target {} \\\n      \
                 --cert-path {} \\\n      \
                 --snap-token \"$(curl -s {}/info | jq -r .auth_token)\"\n",
                network.base_url,
                network.endhost_api_url,
                network.target,
                network.ca_path,
                network.control_url,
            );
        }
        Server::External => {
            let _ = writeln!(out, "  the chat server is yours to start:\n");
            let _ = writeln!(
                out,
                "    cargo run -p chat-server -- {}\n",
                network.chat_server_args.join(" ")
            );
        }
    }

    let _ = writeln!(out, "  Ctrl+C to stop. The network goes with it.\n");
}

/// Resolves when standard input reaches its end.
///
/// A blocking read cannot be cancelled, so the task is left parked and the process exits around it.
async fn stdin_closed() {
    let _ = tokio::task::spawn_blocking(|| {
        let mut line = String::new();
        while std::io::stdin().read_line(&mut line).unwrap_or(0) > 0 {
            line.clear();
        }
    })
    .await;
}
