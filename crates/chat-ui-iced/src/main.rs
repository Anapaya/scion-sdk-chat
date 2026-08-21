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
//! A desktop chat client on iced, for the bake-off.

mod app;
mod chat;
mod connection;
mod sign_in;
mod ui;

use app::App;

fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("scion chat")
        .theme(App::theme)
        .subscription(App::subscription)
        .window_size((900.0, 600.0))
        .run()
}
