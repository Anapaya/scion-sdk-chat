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

use clap::Parser;
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
/// Unparsed strings, because this screen is where a bad value has somewhere to be reported.
///
/// The command line fills the same fields, so it is the same type: a flag left out leaves the
/// default the screen would have shown, and a launch can arrive with the form already answered.
///
/// The variables are `CHAT_CLIENT_*` rather than `CHAT_*` because `chat-server` already reads
/// `CHAT_ENDHOST_API`. The server sits in one AS and this client attaches to another, so a shared
/// variable would point the client at the wrong endhost API.
// The summary is given rather than taken from the doc comment above, which explains the type to a
// reader of this file. `long_about = None` is what stops `--help` printing all four paragraphs of
// it: without it clap takes the doc comment for the long help and the summary is only seen at `-h`.
#[derive(Debug, Clone, PartialEq, Eq, Parser)]
#[command(
    version,
    about = "A terminal chat client, over TCP or over SCION.",
    long_about = None,
)]
pub struct ConnectionForm {
    /// Where the server is. The scheme picks the transport — `http` plain, `https` over SCION.
    #[arg(long, env = "CHAT_CLIENT_SERVER_URL", default_value = DEV_SERVER_URL)]
    pub server_url: String,

    /// The endhost API the client finds SCION through. Required by an `https` URL.
    ///
    /// The client's own AS, not the server's. A local `chat-dev` prints both.
    #[arg(long, env = "CHAT_CLIENT_ENDHOST_API", default_value = "")]
    pub endhost_api: String,

    /// The server's SCION address, for a host with no TSAR record.
    #[arg(long, env = "CHAT_CLIENT_TARGET", default_value = "")]
    pub target: String,

    /// A certificate to trust instead of the system roots.
    #[arg(long, env = "CHAT_CLIENT_CERT_PATH", default_value = "")]
    pub cert_path: String,

    /// The token the SNAP underlay asks for.
    ///
    /// Better given as `CHAT_CLIENT_SNAP_TOKEN`: an argument is readable by anyone who can list
    /// processes.
    #[arg(long, env = "CHAT_CLIENT_SNAP_TOKEN", default_value = "")]
    pub snap_token: String,
}

impl Default for ConnectionForm {
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

/// The form being typed into, and why the last attempt failed.
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
    pub fn new(form: ConnectionForm) -> Self {
        Self {
            server_url: Input::new(form.server_url),
            endhost_api: Input::new(form.endhost_api),
            target: Input::new(form.target),
            cert_path: Input::new(form.cert_path),
            snap_token: Input::new(form.snap_token),
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

    /// Returns the form to connect with once Enter is pressed, and edits a field otherwise.
    ///
    /// `pending` refuses Enter while a call is out. Moving between the fields and typing into them
    /// are this screen's own and always work.
    pub fn handle_key(&mut self, key: KeyEvent, pending: bool) -> Option<ConnectionForm> {
        match key.code {
            KeyCode::Enter => {
                if pending {
                    return None;
                }
                self.error = None;
                return Some(self.form());
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
    fn form(&self) -> ConnectionForm {
        ConnectionForm {
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

#[cfg(test)]
mod tests {
    use clap::Parser as _;

    use super::*;

    #[test]
    fn the_command_line_is_well_formed() {
        <ConnectionForm as clap::CommandFactory>::command().debug_assert();
    }

    /// A launch with no arguments is still the development launch it always was, and the screen
    /// opens on the same defaults it would have shown.
    #[test]
    fn nothing_given_leaves_the_defaults() {
        let form = ConnectionForm::parse_from(["chat-ui-ratatui"]);

        assert_eq!(form, ConnectionForm::default());
    }

    #[test]
    fn every_field_of_the_form_has_a_flag() {
        let form = ConnectionForm::parse_from([
            "chat-ui-ratatui",
            "--server-url",
            "https://localhost:8443",
            "--endhost-api",
            "http://127.0.0.1:41234/",
            "--target",
            "2-ff00:0:212,127.0.0.1",
            "--cert-path",
            "/tmp/dev/cert.pem",
            "--snap-token",
            "a token",
        ]);

        assert_eq!(
            form,
            ConnectionForm {
                server_url: "https://localhost:8443".to_owned(),
                endhost_api: "http://127.0.0.1:41234/".to_owned(),
                target: "2-ff00:0:212,127.0.0.1".to_owned(),
                cert_path: "/tmp/dev/cert.pem".to_owned(),
                snap_token: "a token".to_owned(),
            }
        );
    }
}
