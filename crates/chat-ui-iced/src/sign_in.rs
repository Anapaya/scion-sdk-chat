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
//! Who is signing in.

use iced::{
    Element, Length,
    widget::{button, column, container, row, text, text_input},
};

use crate::{app::Message, ui::error_line};

/// The sign-in screen's fields.
#[derive(Default)]
pub struct SignIn {
    pub username: String,
    pub password: String,
    pub error: Option<String>,
    /// A call is out, so neither button is pressable again.
    pub busy: bool,
}

impl SignIn {
    /// Whether there is enough to send. Both calls need both fields.
    fn ready(&self) -> bool {
        !self.busy && !self.username.is_empty() && !self.password.is_empty()
    }
}

pub fn view(state: &SignIn) -> Element<'_, Message> {
    let log_in = state.ready().then_some(Message::LogIn);
    let register = state.ready().then_some(Message::Register);

    let form = column![
        text("Sign in").size(28),
        text_input("username", &state.username)
            .on_input(Message::UsernameEdited)
            .on_submit(Message::LogIn)
            .padding(10),
        text_input("password", &state.password)
            .secure(true)
            .on_input(Message::PasswordEdited)
            .on_submit(Message::LogIn)
            .padding(10),
        row![
            button(text("Register"))
                .on_press_maybe(register)
                .style(button::secondary)
                .padding([8, 16]),
            button(text("Log in"))
                .on_press_maybe(log_in)
                .padding([8, 16]),
        ]
        .spacing(12),
        error_line(state.error.as_deref()),
    ]
    .spacing(16)
    .max_width(420);

    container(form)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into()
}
