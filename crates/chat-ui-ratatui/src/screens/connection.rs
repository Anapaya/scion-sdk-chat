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
//! Where the server is. The first screen, because every launch starts here.

use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Flex, Layout, Rect},
    style::Stylize,
    text::Line,
};
use tui_input::{Input, backend::crossterm::EventHandler as _};

use crate::ui::{self, field, layout::form, theme};

/// The address the server listens on in development mode, so the common case is one keypress.
const DEV_SERVER_URL: &str = "http://localhost:8080";

/// The server URL being typed, and why the last attempt failed.
pub struct Connection {
    url: Input,
    /// What went wrong last time, shown until the next attempt.
    pub error: Option<String>,
}

impl Default for Connection {
    fn default() -> Self {
        Self {
            url: Input::new(DEV_SERVER_URL.to_owned()),
            error: None,
        }
    }
}

impl Connection {
    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let [title, url, hint, error] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .flex(Flex::Center)
        .areas(form(area));

        frame.render_widget(Line::from("Connect".fg(theme::TITLE).bold()), title);
        field::draw(
            frame,
            url,
            ui::label(" Server URL "),
            &self.url,
            true,
            false,
        );
        frame.render_widget(
            Line::from(vec![
                " Connect ".fg(theme::DIM),
                "<Enter>".fg(theme::FOCUS).bold(),
            ])
            .right_aligned(),
            hint,
        );
        if let Some(message) = &self.error {
            ui::draw_error(frame, error, message);
        }
    }

    /// Returns the URL to connect to once Enter is pressed, and edits the field otherwise.
    ///
    /// `pending` refuses Enter while a call is out. Typing is this screen's own and always works.
    pub fn handle_key(&mut self, key: KeyEvent, pending: bool) -> Option<String> {
        if key.code == KeyCode::Enter {
            if pending {
                return None;
            }
            self.error = None;
            return Some(self.url.value().trim().to_owned());
        }

        self.url.handle_event(&Event::Key(key));
        None
    }
}
