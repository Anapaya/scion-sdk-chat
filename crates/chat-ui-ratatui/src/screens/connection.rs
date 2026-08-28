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
//! Where the server is, and how to reach it. The first screen, because every launch starts here.

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

/// Everything the client needs to reach a server, as typed.
///
/// Unparsed, because this screen is where a bad value has somewhere to be reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Where the server is. The scheme picks the transport — `http` plain, `https` over SCION.
    pub server_url: String,
    /// The endhost API the client finds SCION through.
    pub endhost_api: String,
    /// The server's SCION address, for a host with no TSAR record.
    pub target: String,
    /// A certificate to trust instead of the system roots.
    pub cert_path: String,
    /// The token the SNAP underlay asks for.
    pub snap_token: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server_url: DEV_SERVER_URL.to_owned(),
            endhost_api: String::new(),
            target: String::new(),
            cert_path: String::new(),
            snap_token: String::new(),
        }
    }
}

/// Which field the keys are going to.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Focus {
    #[default]
    ServerUrl,
    EndhostApi,
    Target,
    CertPath,
    SnapToken,
}

impl Focus {
    /// The field after this one. Wraps.
    fn next(self) -> Self {
        match self {
            Self::ServerUrl => Self::EndhostApi,
            Self::EndhostApi => Self::Target,
            Self::Target => Self::CertPath,
            Self::CertPath => Self::SnapToken,
            Self::SnapToken => Self::ServerUrl,
        }
    }

    /// The field before this one. Wraps.
    fn previous(self) -> Self {
        match self {
            Self::ServerUrl => Self::SnapToken,
            Self::EndhostApi => Self::ServerUrl,
            Self::Target => Self::EndhostApi,
            Self::CertPath => Self::Target,
            Self::SnapToken => Self::CertPath,
        }
    }
}

/// The settings being typed, and why the last attempt failed.
pub struct Connection {
    server_url: Input,
    endhost_api: Input,
    target: Input,
    cert_path: Input,
    snap_token: Input,
    focus: Focus,
    /// What went wrong last time, shown until the next attempt.
    pub error: Option<String>,
}

impl Connection {
    /// The screen with its fields already answered, as the command line answers them.
    pub fn new(settings: Settings) -> Self {
        Self {
            server_url: Input::new(settings.server_url),
            endhost_api: Input::new(settings.endhost_api),
            target: Input::new(settings.target),
            cert_path: Input::new(settings.cert_path),
            snap_token: Input::new(settings.snap_token),
            focus: Focus::default(),
            error: None,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let [
            title,
            schemes,
            url,
            endhost,
            target,
            cert,
            token,
            hint,
            error,
        ] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .flex(Flex::Center)
        .areas(form(area));

        frame.render_widget(Line::from("Connect".fg(theme::TITLE).bold()), title);
        frame.render_widget(
            Line::from("http for TCP  ·  https for SCION".fg(theme::DIM)),
            schemes,
        );

        for (area, label, input, focus, mask) in [
            (
                url,
                " Server URL ",
                &self.server_url,
                Focus::ServerUrl,
                false,
            ),
            (
                endhost,
                " Endhost API ",
                &self.endhost_api,
                Focus::EndhostApi,
                false,
            ),
            // The URL carries the name the certificate is issued for; this carries the address
            // that name is not resolved to.
            (
                target,
                " Target - the server's SCION address ",
                &self.target,
                Focus::Target,
                false,
            ),
            (
                cert,
                " Certificate ",
                &self.cert_path,
                Focus::CertPath,
                false,
            ),
            (
                token,
                " SNAP token ",
                &self.snap_token,
                Focus::SnapToken,
                true,
            ),
        ] {
            field::draw(
                frame,
                area,
                ui::label(label),
                input,
                self.focus == focus,
                mask,
            );
        }

        frame.render_widget(
            Line::from(vec![
                " Connect ".fg(theme::DIM),
                "<Enter>".fg(theme::FOCUS).bold(),
                "  Next field ".fg(theme::DIM),
                "<Tab>".fg(theme::FOCUS).bold(),
            ])
            .right_aligned(),
            hint,
        );
        if let Some(message) = &self.error {
            ui::draw_error(frame, error, message);
        }
    }

    /// Returns the settings to connect with once Enter is pressed, and edits a field otherwise.
    ///
    /// `pending` refuses Enter while a call is out. Moving between the fields and typing into them
    /// are this screen's own and always work.
    pub fn handle_key(&mut self, key: KeyEvent, pending: bool) -> Option<Settings> {
        match key.code {
            KeyCode::Enter => {
                if pending {
                    return None;
                }
                self.error = None;
                return Some(self.settings());
            }
            KeyCode::Tab | KeyCode::Down => self.focus = self.focus.next(),
            KeyCode::BackTab | KeyCode::Up => self.focus = self.focus.previous(),
            _ => {
                self.focused_mut().handle_event(&Event::Key(key));
            }
        }
        None
    }

    /// Every field as typed, trimmed. A blank one stays blank, which the app reads as an absence.
    fn settings(&self) -> Settings {
        Settings {
            server_url: self.server_url.value().trim().to_owned(),
            endhost_api: self.endhost_api.value().trim().to_owned(),
            target: self.target.value().trim().to_owned(),
            cert_path: self.cert_path.value().trim().to_owned(),
            snap_token: self.snap_token.value().trim().to_owned(),
        }
    }

    fn focused_mut(&mut self) -> &mut Input {
        match self.focus {
            Focus::ServerUrl => &mut self.server_url,
            Focus::EndhostApi => &mut self.endhost_api,
            Focus::Target => &mut self.target,
            Focus::CertPath => &mut self.cert_path,
            Focus::SnapToken => &mut self.snap_token,
        }
    }
}
