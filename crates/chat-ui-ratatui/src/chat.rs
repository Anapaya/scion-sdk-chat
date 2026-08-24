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
//! Rooms on the left, the open room's messages on the right, a line to type at the bottom.

use std::collections::HashMap;

use chat_client_core::v1::{Message, Room, RoomId, Seq};
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, List, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
};
use tui_input::{Input, backend::crossterm::EventHandler as _};

use crate::{field, theme};

/// How wide the room list is. Fixed, because a room name is short and the messages want the rest.
const SIDEBAR_WIDTH: u16 = 18;

/// How much room a name is given before the message it sent.
///
/// Also the longest name this client will register, since a wider one would push its own messages
/// out of line with every other row. The server accepts more; this is the limit of what the pane
/// can draw tidily.
pub const NAME_WIDTH: usize = 10;

/// The longest room name this client will create.
///
/// What the sidebar can hold: [`SIDEBAR_WIDTH`] less its two borders, the highlight arrow, the `#`,
/// and the unread dot leaves eleven columns for the name itself.
const ROOM_NAME_MAX: usize = 10;

/// The word that turns a line into a room rather than a message.
const CREATE: &str = "/room";

/// The word that lists what the other words do.
const HELP: &str = "/help";

/// What the screen is asking for.
pub enum Intent {
    /// Post what was typed to the open room.
    Send(String),
    /// Create a room under this name.
    Create(String),
    /// Read the room the sidebar has just moved to.
    Open,
}

/// The rooms, the open room's messages, and the line being typed.
pub struct Chat {
    /// Every room the server listed, in the order the sidebar shows them.
    rooms: Vec<Room>,
    /// Which room is open.
    open: ListState,
    /// The open room's messages, oldest first.
    messages: Vec<Message>,
    /// Lines this client wrote itself, drawn under the messages. Nobody else sees them and the
    /// server never hears about them, which is what makes them the right place for `/help`.
    notices: Vec<Line<'static>>,
    /// The newest `seq` the user has actually seen in each room. Only the room on screen advances
    /// it, which is what makes it the badge cursor rather than a resume cursor.
    last_read: HashMap<RoomId, Seq>,
    /// The room a feed is watching, which is therefore being read.
    watched: Option<RoomId>,
    /// How far down the pane is scrolled, or `None` to follow the newest message.
    scroll: Option<u16>,
    /// What the last draw measured: the largest offset, and how many rows fit. Scrolling by a
    /// screenful needs both, and only a draw knows the pane's size.
    measured: (u16, u16),
    /// Who is logged in, so their own name is drawn apart from everyone else's.
    me: String,
    input: Input,
    /// Why the last call failed, shown until one works.
    pub error: Option<String>,
}

impl Chat {
    /// Opens over the rooms the server listed, with the first one — always the lobby — selected.
    pub fn new(rooms: Vec<Room>, me: String) -> Self {
        let last_read = read_from(&rooms);

        Self {
            rooms,
            open: ListState::default().with_selected(Some(0)),
            messages: Vec::new(),
            notices: Vec::new(),
            last_read,
            watched: None,
            scroll: None,
            measured: (0, 0),
            me,
            input: Input::default(),
            error: None,
        }
    }

    /// The room the sidebar has selected, if the server listed any.
    pub fn open_room(&self) -> Option<&Room> {
        self.rooms.get(self.selected())
    }

    /// Replaces the sidebar, keeping the open room selected if the server still lists it.
    pub fn show_rooms(&mut self, rooms: Vec<Room>) {
        let open = self.open_room().map(|room| room.id);
        self.rooms = rooms;

        let index = open
            .and_then(|id| self.rooms.iter().position(|room| room.id == id))
            .unwrap_or(0);
        self.open.select(Some(index));
    }

    /// Appends a batch the feed delivered, and marks the open room read up to it.
    ///
    /// Batches arrive oldest first and never overlap, so appending is all there is to do.
    pub fn append(&mut self, messages: Vec<Message>) {
        let newest = messages.last().map(|message| message.seq);
        let open = self.open_room().map(|room| room.id);

        if let (Some(room), Some(seq)) = (open, newest) {
            self.last_read.insert(room, seq);
        }
        self.messages.extend(messages);
    }

    /// Empties the message pane, for a room that is about to be watched.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.notices.clear();
        self.watched = None;
        self.scroll = None;
    }

    /// Records which room a feed is now watching.
    pub fn watching(&mut self, room: RoomId) {
        self.watched = Some(room);
    }

    /// Puts back what a failed send did not deliver, so the text is not lost.
    pub fn restore(&mut self, body: String) {
        self.input = Input::new(body);
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let [sidebar, opened] =
            Layout::horizontal([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(20)])
                .areas(area);
        // The error line is given no row at all when there is nothing to report, so the message
        // pane grows into it rather than the screen carrying a blank row.
        let [messages, composer, error] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Length(u16::from(self.error.is_some())),
        ])
        .areas(opened);

        self.draw_rooms(frame, sidebar);
        let measured = self.draw_messages(frame, messages);
        self.measured = measured;
        field::draw(frame, composer, instructions(), &self.input, true, false);
        if let Some(message) = &self.error {
            frame.render_widget(
                Paragraph::new(format!("⚠ {message}")).fg(theme::ERROR),
                error,
            );
        }
    }

    fn draw_rooms(&mut self, frame: &mut Frame, area: Rect) {
        let names = self.rooms.iter().map(|room| {
            if self.unread(room) {
                Line::from(vec![
                    Span::from(format!("#{}", room.name))
                        .fg(theme::UNREAD)
                        .bold(),
                    Span::from(" ●").fg(theme::UNREAD),
                ])
            } else {
                Line::from(Span::from(format!("#{}", room.name)).fg(theme::TEXT))
            }
        });
        let list = List::new(names)
            .block(panel(" Rooms "))
            .highlight_symbol("▸ ")
            .highlight_style(
                Style::new()
                    .bg(theme::SELECTION)
                    .fg(theme::HIGHLIGHT)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_stateful_widget(list, area, &mut self.open);
    }

    /// Draws the messages, and reports the largest offset and how many rows fit.
    fn draw_messages(&self, frame: &mut Frame, area: Rect) -> (u16, u16) {
        let title = match self.open_room() {
            Some(room) => format!(" #{} ", room.name),
            None => " no rooms ".to_owned(),
        };
        let mut lines: Vec<Line<'_>> = self
            .messages
            .iter()
            .map(|message| {
                Line::from(vec![
                    Span::from(format!("{:>NAME_WIDTH$} ", message.username))
                        .fg(self.colour(&message.username)),
                    Span::from(message.body.as_str()).fg(theme::TEXT),
                ])
            })
            .collect();
        lines.extend(self.notices.iter().cloned());

        let block = panel(&title);
        let inner = block.inner(area);
        let messages = Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .block(block);

        // A message longer than the pane takes several rows, so the newest is found by counting the
        // rows a wrap actually produces rather than the messages. The count includes the block's
        // own two rows, so it is compared against the height that also has them.
        let rows = messages.line_count(inner.width) as u16;
        let bottom = rows.saturating_sub(area.height);
        // Following the newest until the reader says otherwise, and never past the end when the
        // pane has grown a message since they last looked.
        let scroll = self.scroll.unwrap_or(bottom).min(bottom);

        frame.render_widget(messages.scroll((scroll, 0)), area);
        draw_scrollbar(frame, area, inner.height, scroll, bottom);

        (bottom, inner.height)
    }

    /// Claims the keys that mean something here and leaves the rest to the input.
    ///
    /// Tab moves rooms because the composer is the only field on this screen, so there is nothing
    /// for it to move between. That leaves the arrows for the messages, which the composer does
    /// not read either, being one line.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Intent> {
        match key.code {
            KeyCode::Enter => return self.submit(),
            KeyCode::BackTab => return self.open(self.selected().saturating_sub(1)),
            KeyCode::Tab => return self.open(self.selected() + 1),
            KeyCode::Up => self.scroll_by(-1),
            KeyCode::Down => self.scroll_by(1),
            KeyCode::PageUp => self.scroll_by(-self.page()),
            KeyCode::PageDown => self.scroll_by(self.page()),
            _ => {
                self.input.handle_event(&Event::Key(key));
            }
        }
        None
    }

    /// Takes what was typed, leaving the line empty. A blank line does nothing.
    ///
    /// A line starting with [`CREATE`] names a room instead of saying something.
    fn submit(&mut self) -> Option<Intent> {
        let typed = self.input.value_and_reset();
        let typed = typed.trim();

        if typed.is_empty() {
            return None;
        }

        let (first, rest) = typed.split_once(char::is_whitespace).unwrap_or((typed, ""));

        if first == HELP {
            self.notices = help();
            // The list is written at the end of the pane, so following the newest is what puts it
            // on screen for a reader who had scrolled up to ask for it.
            self.scroll = None;
            return None;
        }

        // Anything else is the reader moving on, and the list has been read.
        self.notices.clear();

        if first != CREATE {
            return Some(Intent::Send(typed.to_owned()));
        }

        // The line is split on whitespace to find the command, so a name holding any would be cut
        // in half rather than created.
        let name = rest.trim();
        if name.is_empty() || name.contains(char::is_whitespace) {
            self.warn(format!("a room name is one word: {CREATE} scion"));
            return None;
        }
        if name.chars().count() > ROOM_NAME_MAX {
            self.warn(format!(
                "room name is too long - {ROOM_NAME_MAX} characters max"
            ));
            return None;
        }

        Some(Intent::Create(name.to_owned()))
    }

    /// Complains about the line that was typed, in the pane rather than on the error row.
    ///
    /// The error row is cleared by the next call that works, which says nothing about a line the
    /// server was never asked to accept. As a notice it stays until the next line instead.
    fn warn(&mut self, message: String) {
        self.notices = vec![Line::from(format!("⚠ {message}")).fg(theme::ERROR)];
        self.scroll = None;
    }

    /// Moves the pane by `rows`, up when negative, and resumes following the newest on reaching the
    /// end.
    fn scroll_by(&mut self, rows: i32) {
        let bottom = i32::from(self.measured.0);
        let from = i32::from(self.scroll.unwrap_or(self.measured.0));
        let to = from.saturating_add(rows).clamp(0, bottom);

        // Landing on the end gives up the fixed offset rather than holding it, so a message posted
        // afterwards still arrives on screen.
        self.scroll = (to < bottom).then_some(to as u16);
    }

    /// How many rows the last draw fitted, which is what a page means here.
    fn page(&self) -> i32 {
        i32::from(self.measured.1)
    }

    /// Selects a room, stopping at the last one rather than wrapping.
    ///
    /// `ListState::select_next` would count past the end — the list only clamps that while it
    /// draws, and this index is what reaches into `rooms`.
    fn open(&mut self, index: usize) -> Option<Intent> {
        let index = index.min(self.rooms.len().saturating_sub(1));
        if index == self.selected() {
            return None;
        }

        self.open.select(Some(index));
        self.messages.clear();

        Some(Intent::Open)
    }

    /// Whether a room holds anything the user has not seen.
    ///
    /// A yes or no, never a count: `seq` is assigned server-wide, so the gap between two of them
    /// includes messages posted to other rooms. Counting needs that room's messages.
    fn unread(&self, room: &Room) -> bool {
        if self.watched == Some(room.id) {
            return false;
        }

        let last_read = self.last_read.get(&room.id).copied().unwrap_or(Seq::START);

        room.latest_seq > last_read
    }

    /// The colour a name is drawn in: one of everyone else's, or the one kept for whoever is logged
    /// in.
    fn colour(&self, username: &str) -> Color {
        if username == self.me {
            theme::OWN_NICKNAME
        } else {
            theme::nickname(username)
        }
    }

    fn selected(&self) -> usize {
        self.open.selected().unwrap_or(0)
    }
}

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

/// The `/help` output: a heading where a sender's name would go, then one command per line, lined
/// up under the message column rather than the name column.
fn help() -> Vec<Line<'static>> {
    let heading = Line::from(format!("{HELP:>NAME_WIDTH$} commands")).fg(theme::DIM);
    let commands = COMMANDS.map(|(command, does)| {
        Line::from(format!(
            "{blank:NAME_WIDTH$} {command:COMMAND_WIDTH$}{does}",
            blank = ""
        ))
        .fg(theme::DIM)
    });

    std::iter::once(heading).chain(commands).collect()
}

/// What the composer can do, in the shape the forms use: the label dim, the key lit.
fn instructions() -> Line<'static> {
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

/// Draws how far down the pane is, on its own right border.
///
/// `offset` is a count of rows scrolled past and `bottom` the largest it reaches, so `bottom + 1`
/// is how many positions there are and `visible` is how many of them a thumb covers. Nothing is
/// drawn for a pane that fits, where a thumb would fill the track and say nothing.
fn draw_scrollbar(frame: &mut Frame, area: Rect, visible: u16, offset: u16, bottom: u16) {
    if bottom == 0 {
        return;
    }

    let mut state = ScrollbarState::new(bottom as usize + 1)
        .position(offset as usize)
        .viewport_content_length(visible as usize);

    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            // The arrow heads would land on the rounded corners, so the track is inset past them
            // and drawn without heads instead.
            .begin_symbol(None)
            .end_symbol(None)
            .track_style(Style::new().fg(theme::BORDER))
            .thumb_style(Style::new().fg(theme::FOCUS)),
        area.inner(Margin::new(0, 1)),
        &mut state,
    );
}

/// A panel: rounded, named, and a shade lighter than the screen behind it.
fn panel(title: &str) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(theme::BORDER))
        .title(Span::from(title).fg(theme::TITLE).bold())
        .style(Style::new().bg(theme::PANEL))
}

/// Marks every room read as it stands, so a fresh launch starts quiet rather than claiming the
/// whole history is new.
fn read_from(rooms: &[Room]) -> HashMap<RoomId, Seq> {
    rooms
        .iter()
        .map(|room| (room.id, room.latest_seq))
        .collect()
}
