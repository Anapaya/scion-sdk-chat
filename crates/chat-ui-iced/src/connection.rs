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
//! Where the server is.

use iced::{
    Element, Length,
    widget::{button, column, container, text, text_input},
};

use crate::{app::Message, ui::error_line};

/// The dev server, so the common case is one keypress.
const DEV_SERVER_URL: &str = "http://127.0.0.1:8080";

/// The connection screen's fields.
pub struct Connection {
    pub url: String,
    pub error: Option<String>,
    /// A call is out, so the button is not pressable again.
    pub busy: bool,
}

impl Default for Connection {
    fn default() -> Self {
        Connection {
            url: DEV_SERVER_URL.to_owned(),
            error: None,
            busy: false,
        }
    }
}

pub fn view(state: &Connection) -> Element<'_, Message> {
    let connect = (!state.busy).then_some(Message::Connect);

    let form = column![
        text("Connect").size(28),
        text_input(DEV_SERVER_URL, &state.url)
            .on_input(Message::UrlEdited)
            .on_submit(Message::Connect)
            .padding(10),
        button(text(if state.busy {
            "Connecting…"
        } else {
            "Connect"
        }))
        .on_press_maybe(connect)
        .padding([8, 16]),
        error_line(state.error.as_deref()),
    ]
    .spacing(16)
    .max_width(420);

    container(form)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
