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
//! A desktop chat client on Dioxus, for the bake-off.

mod app;
mod screens;

use dioxus::{
    desktop::{Config, LogicalSize, WindowBuilder},
    prelude::*,
};

use crate::app::{ROOMS_REFRESH, Screen, State};

/// The stylesheet, inline rather than an asset, so the binary needs nothing beside it.
const STYLE: &str = include_str!("style.css");

fn main() {
    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            Config::new().with_window(
                WindowBuilder::new()
                    .with_title("scion chat")
                    .with_inner_size(LogicalSize::new(900.0, 600.0)),
            ),
        )
        .launch(App);
}

#[component]
fn App() -> Element {
    // In context rather than passed down, so a screen five levels deep reads the same state as one
    // at the top.
    let state = use_context_provider(State::new);

    // Restarts whenever `open` changes, cancelling the feed it was running for. That cancellation
    // is what makes "one feed at a time" true without an unsubscribe call.
    //
    // The signal is read here, in the closure, and not in the component around it: `use_resource`
    // runs this closure inside a reactive context, and only the reads it sees there become the
    // dependencies that restart the task. Reading `open` outside and capturing the value leaves the
    // resource with no dependency at all, so it runs once with whatever was current and never
    // again.
    let _feed = use_resource(move || {
        let open = (state.open)();

        async move {
            if let Some(room) = open {
                state.watch(room).await;
            }
        }
    });

    // Only while the chat screen is showing, and never before there is a session.
    let _rooms = use_resource(move || {
        let watching = (state.screen)() == Screen::Chat;

        async move {
            if !watching {
                return;
            }
            // No exit condition of its own: leaving the chat screen writes `screen`, which restarts
            // this resource and cancels the loop along with it.
            loop {
                tokio::time::sleep(ROOMS_REFRESH).await;
                state.refresh_rooms().await;
            }
        }
    });

    rsx! {
        style { "{STYLE}" }
        match (state.screen)() {
            Screen::Connection => rsx! { screens::Connection {} },
            Screen::SignIn => rsx! { screens::SignIn {} },
            Screen::Chat => rsx! { screens::Chat {} },
        }
    }
}
