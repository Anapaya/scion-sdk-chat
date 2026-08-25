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
//! A terminal chat client: connect, sign in, chat.
//!
//! Three screens over `chat-client-core`, against a server in `--transport tcp` mode. The screens
//! draw and read keys; [`app`] holds every call to the client.

mod app;
mod screens;
mod ui;

use std::io;

use crossterm::event::KeyModifiers;

/// The modifier a screen checks for, named once so the screens do not import crossterm's whole
/// keyboard.
pub const CONTROL: KeyModifiers = KeyModifiers::CONTROL;

/// The app provides the runtime. The client never makes one, and never spawns anything of its own.
#[tokio::main]
async fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = app::App::default().run(&mut terminal).await;
    ratatui::restore();

    result
}
