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

use chat_client_core::SnapToken;
use clap::{Parser, ValueEnum};
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

/// A transport the client can reach the server over. The same two the server serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum Transport {
    /// HTTP/3 over SCION.
    #[default]
    Scion,
    /// Plain HTTP over TCP. Development only: no TLS.
    Tcp,
}

impl Transport {
    /// The other one. There are two, so a toggle is the whole of the choice.
    fn other(self) -> Self {
        match self {
            Self::Scion => Self::Tcp,
            Self::Tcp => Self::Scion,
        }
    }

    /// The URL scheme this transport is served under.
    ///
    /// One each, and neither is a preference. Over SCION only HTTP/3 exists, which is always TLS;
    /// over TCP this server has no TLS to offer, so `https` would reach nothing.
    pub fn scheme(self) -> &'static str {
        match self {
            Self::Scion => "https",
            Self::Tcp => "http",
        }
    }

    /// The name the flag takes, so an error can quote what was passed.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scion => "scion",
            Self::Tcp => "tcp",
        }
    }
}

/// The connect screen's fields, filled in by flags or `CHAT_CLIENT_*` variables.
#[derive(Debug, Clone, PartialEq, Eq, Parser)]
#[command(
    version,
    about = "A terminal chat client, over TCP or over SCION.",
    long_about = None,
)]
pub struct ConnectionForm {
    /// The transport to reach the server over. The URL's scheme is checked against it.
    #[arg(
        long,
        env = "CHAT_CLIENT_TRANSPORT",
        value_enum,
        default_value = "scion"
    )]
    pub transport: Transport,

    /// Where the server is.
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
    #[arg(
        long,
        env = "CHAT_CLIENT_SNAP_TOKEN",
        hide_env_values = true,
        default_value = ""
    )]
    pub snap_token: SnapToken,
}

impl Default for ConnectionForm {
    fn default() -> Self {
        Self {
            transport: Transport::default(),
            server_url: DEV_SERVER_URL.to_owned(),
            endhost_api: String::new(),
            target: String::new(),
            cert_path: String::new(),
            snap_token: SnapToken::new(""),
        }
    }
}

/// Which field the keys are going to.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Focus {
    /// First, because it decides what the fields under it are for.
    #[default]
    Transport,
    ServerUrl,
    EndhostApi,
    Target,
    CertPath,
    SnapToken,
}

impl Focus {
    /// The fields Tab moves through, in order.
    fn shown(scion: bool) -> &'static [Self] {
        if scion {
            &[
                Self::Transport,
                Self::ServerUrl,
                Self::EndhostApi,
                Self::Target,
                Self::CertPath,
                Self::SnapToken,
            ]
        } else {
            &[Self::Transport, Self::ServerUrl]
        }
    }

    /// The field after this one. Wraps.
    fn next(self, scion: bool) -> Self {
        self.step(scion, 1)
    }

    /// The field before this one. Wraps.
    fn previous(self, scion: bool) -> Self {
        self.step(scion, -1)
    }

    fn step(self, scion: bool, by: isize) -> Self {
        let shown = Self::shown(scion);
        let at = shown.iter().position(|field| *field == self).unwrap_or(0);
        let len = shown.len() as isize;

        shown[usize::try_from((at as isize + by).rem_euclid(len)).unwrap_or(0)]
    }
}

/// The form being typed into, and why the last attempt failed.
pub struct Connection {
    transport: Transport,
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
            transport: form.transport,
            server_url: Input::new(form.server_url),
            endhost_api: Input::new(form.endhost_api),
            target: Input::new(form.target),
            cert_path: Input::new(form.cert_path),
            snap_token: Input::new(form.snap_token.as_str().to_owned()),
            focus: Focus::default(),
            error: None,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let scion = self.transport == Transport::Scion;

        let [
            title,
            transport,
            url,
            endhost,
            target,
            cert,
            token,
            hint,
            error,
        ] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
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

        // Drawn apart from the loop below: there is nothing to type into it, so it has no `Input`.
        field::choice(
            frame,
            transport,
            ui::label(" Transport "),
            &[("SCION", scion), ("TCP", !scion)],
            self.focus == Focus::Transport,
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
            let state = if self.focus == focus {
                field::State::Focused
            } else if scion || focus == Focus::ServerUrl {
                field::State::Idle
            } else {
                field::State::Disabled
            };

            field::draw(frame, area, ui::label(label), input, state, mask);
        }

        frame.render_widget(
            Line::from(vec![
                " Connect ".fg(theme::DIM),
                "<Enter>".fg(theme::FOCUS).bold(),
                "  Next field ".fg(theme::DIM),
                "<Tab>".fg(theme::FOCUS).bold(),
                "  Choose ".fg(theme::DIM),
                "<←→>".fg(theme::FOCUS).bold(),
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
            KeyCode::Tab | KeyCode::Down => {
                self.focus = self.focus.next(self.transport == Transport::Scion);
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.focus = self.focus.previous(self.transport == Transport::Scion);
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                if self.focus == Focus::Transport =>
            {
                self.transport = self.transport.other();
            }
            _ => {
                if let Some(input) = self.focused_mut() {
                    input.handle_event(&Event::Key(key));
                }
            }
        }
        None
    }

    /// Every field as typed, trimmed.
    fn form(&self) -> ConnectionForm {
        ConnectionForm {
            transport: self.transport,
            server_url: self.server_url.value().trim().to_owned(),
            endhost_api: self.endhost_api.value().trim().to_owned(),
            target: self.target.value().trim().to_owned(),
            cert_path: self.cert_path.value().trim().to_owned(),
            snap_token: SnapToken::new(self.snap_token.value().trim()),
        }
    }

    /// The field the keys are going to, or `None` when it is the one with nothing to type into.
    fn focused_mut(&mut self) -> Option<&mut Input> {
        match self.focus {
            Focus::Transport => None,
            Focus::ServerUrl => Some(&mut self.server_url),
            Focus::EndhostApi => Some(&mut self.endhost_api),
            Focus::Target => Some(&mut self.target),
            Focus::CertPath => Some(&mut self.cert_path),
            Focus::SnapToken => Some(&mut self.snap_token),
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

    #[test]
    fn nothing_given_leaves_the_defaults() {
        let form = ConnectionForm::parse_from(["chat-ui-ratatui"]);

        assert_eq!(form, ConnectionForm::default());
    }

    #[test]
    fn every_field_of_the_form_has_a_flag() {
        let form = ConnectionForm::parse_from([
            "chat-ui-ratatui",
            "--transport",
            "tcp",
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
                transport: Transport::Tcp,
                server_url: "https://localhost:8443".to_owned(),
                endhost_api: "http://127.0.0.1:41234/".to_owned(),
                target: "2-ff00:0:212,127.0.0.1".to_owned(),
                cert_path: "/tmp/dev/cert.pem".to_owned(),
                snap_token: SnapToken::new("a token"),
            }
        );
    }
}
