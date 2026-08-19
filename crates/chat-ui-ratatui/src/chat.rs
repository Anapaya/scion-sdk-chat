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
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, BorderType, List, ListState, Paragraph},
};
use tui_input::{Input, backend::crossterm::EventHandler as _};

use crate::{field, theme};

/// How wide the room list is. Fixed, because a room name is short and the messages want the rest.
const SIDEBAR_WIDTH: u16 = 18;

/// How much room a name is given before the message it sent.
const NAME_WIDTH: usize = 10;

/// The word that turns a line into a room rather than a message.
const CREATE: &str = "/room";

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
    /// The newest `seq` the user has actually seen in each room. Only the room on screen advances
    /// it, which is what makes it the badge cursor rather than a resume cursor.
    last_read: HashMap<RoomId, Seq>,
    /// Who is logged in, so their own name is drawn apart from everyone else's.
    me: String,
    input: Input,
    /// What went wrong last time. Shown until it recovers.
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
            last_read,
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

    /// Replaces what is on screen with the page just fetched, and marks the open room read.
    pub fn show(&mut self, messages: Vec<Message>) {
        if let Some(room) = self.open_room() {
            let (id, latest) = (room.id, room.latest_seq);
            self.last_read.insert(id, latest);
        }
        self.messages = messages;
    }

    /// Puts back what a failed send did not deliver, so the text is not lost.
    pub fn restore(&mut self, body: String) {
        self.input = Input::new(body);
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let [sidebar, opened] =
            Layout::horizontal([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(20)])
                .areas(area);
        let [messages, composer, error] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .areas(opened);

        self.draw_rooms(frame, sidebar);
        self.draw_messages(frame, messages);
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

    fn draw_messages(&self, frame: &mut Frame, area: Rect) {
        let title = match self.open_room() {
            Some(room) => format!(" #{} ", room.name),
            None => " no rooms ".to_owned(),
        };
        let lines = self.messages.iter().map(|message| {
            Line::from(vec![
                Span::from(format!("{:<NAME_WIDTH$}", message.username))
                    .fg(self.colour(&message.username)),
                Span::from(message.body.as_str()).fg(theme::TEXT),
            ])
        });

        frame.render_widget(List::new(lines).block(panel(&title)), area);
    }

    /// Claims the keys that mean something here and leaves the rest to the input.
    ///
    /// Up and Down are free for the sidebar because the composer is one line and does not read
    /// them.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Intent> {
        match key.code {
            KeyCode::Enter => return self.submit(),
            KeyCode::Up => return self.open(self.selected().saturating_sub(1)),
            KeyCode::Down => return self.open(self.selected() + 1),
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
        if first != CREATE {
            return Some(Intent::Send(typed.to_owned()));
        }

        // The line is split on whitespace to find the command, so a name holding any would be cut
        // in half rather than created.
        let name = rest.trim();
        if name.is_empty() || name.contains(char::is_whitespace) {
            self.error = Some(format!("a room name is one word: {CREATE} scion"));
            return None;
        }

        Some(Intent::Create(name.to_owned()))
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

/// What the composer can do, in the shape the forms use: the label dim, the key lit.
fn instructions() -> Line<'static> {
    Line::from(vec![
        " Send ".fg(theme::DIM),
        "<Enter>".fg(theme::FOCUS).bold(),
        "  Room ".fg(theme::DIM),
        "<↑↓>".fg(theme::FOCUS).bold(),
        "  Create ".fg(theme::DIM),
        "</room name>".fg(theme::FOCUS).bold(),
        " ".fg(theme::DIM),
    ])
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
