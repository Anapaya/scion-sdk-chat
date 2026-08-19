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
//! Who is signing in. Registering and logging in are separate, as the API is.

use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::Stylize,
    text::Line,
    widgets::Paragraph,
};
use tui_input::{Input, backend::crossterm::EventHandler as _};

use crate::{field, layout::screen, theme};

/// What the screen is asking for.
pub enum Intent {
    /// Create the account. Deliberately does not sign in — the server keeps the two apart.
    Register,
    /// Sign in and open the chat.
    LogIn,
}

/// Which field the keys are going to.
#[derive(Default, PartialEq, Eq)]
enum Focus {
    #[default]
    Username,
    Password,
}

/// The credentials being typed, and why the last attempt failed.
#[derive(Default)]
pub struct SignIn {
    username: Input,
    password: Input,
    focus: Focus,
    /// What went wrong last time, shown until the next attempt.
    pub error: Option<String>,
}

impl SignIn {
    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let [title, username, password, hint, error] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .flex(Flex::Center)
        .areas(screen(area, 60));

        frame.render_widget(Line::from("Sign in".fg(theme::TITLE).bold()), title);
        field::draw(
            frame,
            username,
            label(" Username "),
            &self.username,
            self.focus == Focus::Username,
            false,
        );
        field::draw(
            frame,
            password,
            label(" Password "),
            &self.password,
            self.focus == Focus::Password,
            true,
        );
        frame.render_widget(
            Line::from(vec![
                " Register ".fg(theme::DIM),
                "<Ctrl+R>".fg(theme::FOCUS).bold(),
                "  Log in ".fg(theme::DIM),
                "<Enter>".fg(theme::FOCUS).bold(),
                "  Next field ".fg(theme::DIM),
                "<Tab>".fg(theme::FOCUS).bold(),
            ])
            .right_aligned(),
            hint,
        );
        if let Some(message) = &self.error {
            frame.render_widget(
                Paragraph::new(format!("⚠ {message}")).fg(theme::ERROR),
                error,
            );
        }
    }

    /// The username and password as typed.
    pub fn credentials(&self) -> (String, String) {
        (
            self.username.value().to_owned(),
            self.password.value().to_owned(),
        )
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Intent> {
        // Registering has no key of its own on the wireframe, and every plain key belongs to a
        // field, so it takes the modifier.
        if key.code == KeyCode::Char('r') && key.modifiers.contains(crate::CONTROL) {
            self.error = None;
            return Some(Intent::Register);
        }

        match key.code {
            KeyCode::Enter => {
                self.error = None;
                return Some(Intent::LogIn);
            }
            KeyCode::Tab | KeyCode::Down => self.focus = Focus::Password,
            KeyCode::BackTab | KeyCode::Up => self.focus = Focus::Username,
            _ => {
                self.focused_mut().handle_event(&Event::Key(key));
            }
        }
        None
    }

    fn focused_mut(&mut self) -> &mut Input {
        match self.focus {
            Focus::Username => &mut self.username,
            Focus::Password => &mut self.password,
        }
    }
}

/// The name of a field, in the colour every screen gives one.
fn label(text: &str) -> Line<'_> {
    Line::from(text.fg(theme::TITLE))
}
