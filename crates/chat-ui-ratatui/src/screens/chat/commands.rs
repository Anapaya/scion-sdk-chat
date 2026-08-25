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
//! What a typed line can mean, and the two places the vocabulary is written out.

use ratatui::{style::Stylize, text::Line};
use unicode_width::UnicodeWidthStr as _;

use super::{Chat, Intent, view::ROOM_NAME_MAX};
use crate::ui::{BODY_COLUMN, NAME_WIDTH, theme};

/// The word that turns a line into a room rather than a message.
const CREATE: &str = "/room";

/// The word that lists what the other words do.
const HELP: &str = "/help";

/// Every command and key `/help` lists, and what each one does.
///
/// Kept beside [`instructions`] because the composer's hint line names a few of the same keys, and
/// the two have to be changed together.
const COMMANDS: [(&str, &str); 8] = [
    (HELP, "list these commands"),
    ("/room <name>", "create a room"),
    ("<Enter>", "send what is typed"),
    ("<Tab>", "next room"),
    ("<Shift+Tab>", "previous room"),
    ("<↑↓>", "scroll a line"),
    ("<PgUp/PgDn>", "scroll a screenful"),
    ("<Esc>/<Ctrl+C>", "quit"),
];

/// How much room a command is given before what it does. The widest is `<Esc>/<Ctrl+C>`.
const COMMAND_WIDTH: usize = 16;

impl Chat {
    /// Takes what was typed, leaving the line empty. A blank line does nothing.
    ///
    /// A line starting with [`CREATE`] names a room instead of saying something.
    ///
    /// The line is read rather than taken until it is clear where it goes: `pending` refuses the
    /// two that reach the server, and a line refused has to stay where it was typed. `/help` and a
    /// name this client will not accept are its own, and answer whatever is out.
    pub(super) fn submit(&mut self, pending: bool) -> Option<Intent> {
        let typed = self.input.value().trim().to_owned();

        if typed.is_empty() {
            self.input.reset();
            return None;
        }

        let (first, rest) = typed
            .split_once(char::is_whitespace)
            .unwrap_or((typed.as_str(), ""));

        if first == HELP {
            self.input.reset();
            self.notices = help();
            // The list is written at the end of the pane, so following the newest is what puts it
            // on screen for a reader who had scrolled up to ask for it.
            self.scroll = None;
            self.changed();
            return None;
        }

        if first != CREATE {
            if pending {
                return None;
            }
            self.input.reset();
            // The reader is moving on, and the list has been read.
            self.notices.clear();
            self.changed();

            return Some(Intent::Send(typed));
        }

        // The line is split on whitespace to find the command, so a name holding any would be cut
        // in half rather than created.
        let name = rest.trim();
        if name.is_empty() || name.contains(char::is_whitespace) {
            self.input.reset();
            self.warn(format!("a room name is one word: {CREATE} scion"));
            return None;
        }
        if name.width() > ROOM_NAME_MAX {
            self.input.reset();
            self.warn(format!(
                "room name is too long - {ROOM_NAME_MAX} characters max"
            ));
            return None;
        }
        if pending {
            return None;
        }

        let name = name.to_owned();
        self.input.reset();
        self.notices.clear();
        self.changed();

        Some(Intent::Create(name))
    }
}

/// The `/help` output: a heading where a sender's name would go, then one command per line, lined
/// up under the message column rather than the name column.
fn help() -> Vec<Line<'static>> {
    let heading = Line::from(format!("{HELP:>NAME_WIDTH$} commands")).fg(theme::DIM);
    let commands = COMMANDS.map(|(command, does)| {
        Line::from(format!(
            "{blank:BODY_COLUMN$}{command:COMMAND_WIDTH$}{does}",
            blank = ""
        ))
        .fg(theme::DIM)
    });

    std::iter::once(heading).chain(commands).collect()
}

/// What the composer can do, in the shape the forms use: the label dim, the key lit.
pub(super) fn instructions() -> Line<'static> {
    Line::from(vec![
        " Send ".fg(theme::DIM),
        "<Enter>".fg(theme::FOCUS).bold(),
        "  Room ".fg(theme::DIM),
        "<Tab>".fg(theme::FOCUS).bold(),
        "  Create ".fg(theme::DIM),
        "</room name>".fg(theme::FOCUS).bold(),
        "  Help ".fg(theme::DIM),
        HELP.fg(theme::FOCUS).bold(),
        " ".fg(theme::DIM),
    ])
}
