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
    /// The other one.
    fn other(self) -> Self {
        match self {
            Self::Scion => Self::Tcp,
            Self::Tcp => Self::Scion,
        }
    }

    /// The URL scheme this transport is served under.
    ///
    /// SCION carries only HTTP/3, which is always TLS. This server offers no TLS over TCP.
    pub fn scheme(self) -> &'static str {
        match self {
            Self::Scion => "https",
            Self::Tcp => "http",
        }
    }

    /// The name the flag takes.
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

    /// The endhost API the client finds SCION through, in its own AS. Required by an `https` URL.
    #[arg(long, env = "CHAT_CLIENT_ENDHOST_API", default_value = "")]
    pub endhost_api: String,

    /// The server's SCION address, for a host with no TSAR record.
    #[arg(long, env = "CHAT_CLIENT_TARGET", default_value = "")]
    pub target: String,

    /// A certificate to trust instead of the system roots.
    #[arg(long, env = "CHAT_CLIENT_CERT_PATH", default_value = "")]
    pub cert_path: String,

    /// Accept any certificate the server presents.
    ///
    /// Anyone on the path can then answer as the server. Use it against a server you control,
    /// whose certificate you have not copied here yet.
    #[arg(long, env = "CHAT_CLIENT_INSECURE")]
    pub insecure: bool,

    /// The token the SNAP underlay asks for. An argument is readable by anyone listing processes.
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
            insecure: false,
            snap_token: SnapToken::new(""),
        }
    }
}

/// Which certificates the client accepts, as the form offers the choice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum TrustChoice {
    /// The anchors the operating system ships.
    #[default]
    System,
    /// One certificate, named in the field below.
    Pinned,
    /// Any certificate at all.
    Insecure,
}

impl TrustChoice {
    /// What the form was launched with. A named certificate is what asks for pinning.
    fn of(form: &ConnectionForm) -> Self {
        if form.insecure {
            Self::Insecure
        } else if form.cert_path.is_empty() {
            Self::System
        } else {
            Self::Pinned
        }
    }

    /// The next choice, wrapping, in the order the options are drawn.
    fn next(self) -> Self {
        match self {
            Self::System => Self::Pinned,
            Self::Pinned => Self::Insecure,
            Self::Insecure => Self::System,
        }
    }

    /// The previous choice, wrapping.
    fn previous(self) -> Self {
        self.next().next()
    }
}

/// Which field the keys are going to.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
enum Focus {
    /// First: it decides what the fields under it are for.
    #[default]
    Transport,
    ServerUrl,
    EndhostApi,
    Target,
    Trust,
    CertPath,
    SnapToken,
}

impl Focus {
    /// The fields Tab moves through, in order. The certificate is only asked for when it is used.
    fn shown(scion: bool, pinned: bool) -> &'static [Self] {
        match (scion, pinned) {
            (false, _) => &[Self::Transport, Self::ServerUrl],
            (true, false) => {
                &[
                    Self::Transport,
                    Self::ServerUrl,
                    Self::EndhostApi,
                    Self::Target,
                    Self::Trust,
                    Self::SnapToken,
                ]
            }
            (true, true) => {
                &[
                    Self::Transport,
                    Self::ServerUrl,
                    Self::EndhostApi,
                    Self::Target,
                    Self::Trust,
                    Self::CertPath,
                    Self::SnapToken,
                ]
            }
        }
    }

    /// The field after this one. Wraps.
    fn next(self, scion: bool, pinned: bool) -> Self {
        self.step(scion, pinned, 1)
    }

    /// The field before this one. Wraps.
    fn previous(self, scion: bool, pinned: bool) -> Self {
        self.step(scion, pinned, -1)
    }

    fn step(self, scion: bool, pinned: bool, by: isize) -> Self {
        let shown = Self::shown(scion, pinned);
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
    trust: TrustChoice,
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
            trust: TrustChoice::of(&form),
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
        let pinned = self.trust == TrustChoice::Pinned;

        let [
            title,
            transport,
            url,
            endhost,
            target,
            trust,
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
            // The certificate is only asked for when it is used. The row still exists when
            // verification is off, to carry the warning.
            Constraint::Length(match self.trust {
                TrustChoice::Pinned => 3,
                TrustChoice::Insecure => 1,
                TrustChoice::System => 0,
            }),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .flex(Flex::Center)
        .areas(form(area));

        frame.render_widget(Line::from("Connect".fg(theme::TITLE).bold()), title);

        // Drawn apart from the loop below: there is nothing to type into them, so they have no
        // `Input`.
        field::choice(
            frame,
            transport,
            ui::label(" Transport "),
            &[("SCION", scion), ("TCP", !scion)],
            self.focus == Focus::Transport,
        );
        field::choice(
            frame,
            trust,
            ui::label(" Trust "),
            &[
                ("System roots", self.trust == TrustChoice::System),
                ("Pinned", pinned),
                ("No check", self.trust == TrustChoice::Insecure),
            ],
            self.focus == Focus::Trust,
        );
        if self.trust == TrustChoice::Insecure {
            frame.render_widget(
                Line::from(" Any server on the path can answer as this one.".fg(theme::ERROR)),
                cert,
            );
        }

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
            if focus == Focus::CertPath && !pinned {
                continue;
            }

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
                self.focus = self.focus.next(self.scion(), self.pinned());
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.focus = self.focus.previous(self.scion(), self.pinned());
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ')
                if self.focus == Focus::Transport =>
            {
                self.transport = self.transport.other();
            }
            KeyCode::Left if self.focus == Focus::Trust => self.trust = self.trust.previous(),
            KeyCode::Right | KeyCode::Char(' ') if self.focus == Focus::Trust => {
                self.trust = self.trust.next();
            }
            _ => {
                if let Some(input) = self.focused_mut() {
                    input.handle_event(&Event::Key(key));
                }
            }
        }
        None
    }

    fn scion(&self) -> bool {
        self.transport == Transport::Scion
    }

    fn pinned(&self) -> bool {
        self.trust == TrustChoice::Pinned
    }

    /// Every field as typed, trimmed.
    ///
    /// The certificate goes out only when it is the choice, so the two ways of naming a trust can
    /// never disagree.
    fn form(&self) -> ConnectionForm {
        ConnectionForm {
            transport: self.transport,
            server_url: self.server_url.value().trim().to_owned(),
            endhost_api: self.endhost_api.value().trim().to_owned(),
            target: self.target.value().trim().to_owned(),
            insecure: self.trust == TrustChoice::Insecure,
            cert_path: if self.pinned() {
                self.cert_path.value().trim().to_owned()
            } else {
                String::new()
            },
            snap_token: SnapToken::new(self.snap_token.value().trim()),
        }
    }

    /// The field the keys are going to, or `None` when it is the one with nothing to type into.
    fn focused_mut(&mut self) -> Option<&mut Input> {
        match self.focus {
            Focus::Transport | Focus::Trust => None,
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
                insecure: false,
                snap_token: SnapToken::new("a token"),
            }
        );
    }

    /// A launch names its trust one way or the other, and the screen opens on that choice.
    #[test]
    fn the_form_opens_on_the_trust_the_flags_asked_for() {
        let pinned = ConnectionForm {
            cert_path: "/tmp/dev/cert.pem".to_owned(),
            ..ConnectionForm::default()
        };
        let insecure = ConnectionForm {
            insecure: true,
            ..ConnectionForm::default()
        };

        assert_eq!(
            TrustChoice::of(&ConnectionForm::default()),
            TrustChoice::System
        );
        assert_eq!(TrustChoice::of(&pinned), TrustChoice::Pinned);
        assert_eq!(TrustChoice::of(&insecure), TrustChoice::Insecure);
    }

    /// The certificate goes out only when it is the choice, so the flags can never disagree.
    #[test]
    fn leaving_the_pinned_choice_drops_the_certificate() {
        let mut screen = Connection::new(ConnectionForm {
            cert_path: "/tmp/dev/cert.pem".to_owned(),
            ..ConnectionForm::default()
        });
        assert_eq!(screen.form().cert_path, "/tmp/dev/cert.pem");

        screen.trust = TrustChoice::Insecure;

        let form = screen.form();
        assert!(form.insecure);
        assert_eq!(form.cert_path, "", "a path that is not used is not sent");
    }

    /// Tab reaches the certificate only when it is going to be read.
    #[test]
    fn the_certificate_field_is_only_in_the_tab_order_when_it_is_pinned() {
        assert!(!Focus::shown(true, false).contains(&Focus::CertPath));
        assert!(Focus::shown(true, true).contains(&Focus::CertPath));
        assert!(Focus::shown(true, false).contains(&Focus::Trust));
        assert!(!Focus::shown(false, true).contains(&Focus::Trust));
    }
}
