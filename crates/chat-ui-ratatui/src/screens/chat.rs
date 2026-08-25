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
//!
//! What the screen holds and which keys mean what. [`view`] draws it and [`commands`] reads the
//! line that was typed.

use std::collections::HashMap;

use chat_client_core::v1::{Message, Room, RoomId, Seq};
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{style::Color, text::Line, widgets::ListState};
use tui_input::{Input, backend::crossterm::EventHandler as _};

use crate::ui::{self, theme};

mod commands;
mod view;

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
    /// The message pane as it was last drawn.
    pane: Option<view::Pane>,
    /// Counts the changes to what the pane shows, which is what a kept one is checked against.
    ///
    /// The room list is not counted: it is re-read every couple of seconds, and the pane's title
    /// follows the room rather than its place in the list.
    revision: u64,
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
            pane: None,
            revision: 0,
        }
    }

    /// Records that what the pane shows has changed, so the next draw builds it again.
    fn changed(&mut self) {
        self.revision += 1;
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
        self.changed();
    }

    /// Empties the message pane, for a room that is about to be watched.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.notices.clear();
        self.watched = None;
        self.scroll = None;
        self.changed();
    }

    /// Records which room a feed is now watching.
    pub fn watching(&mut self, room: RoomId) {
        self.watched = Some(room);
    }

    /// Puts back what a failed send did not deliver, so the text is not lost.
    pub fn restore(&mut self, body: String) {
        self.input = Input::new(body);
    }

    /// Claims the keys that mean something here and leaves the rest to the input.
    ///
    /// Tab moves rooms because the composer is the only field on this screen, so there is nothing
    /// for it to move between. That leaves the arrows for the messages, which the composer does
    /// not read either, being one line.
    /// `pending` refuses the keys that reach the server. Scrolling and typing are this screen's
    /// own and always work.
    pub fn handle_key(&mut self, key: KeyEvent, pending: bool) -> Option<Intent> {
        match key.code {
            KeyCode::Enter => return self.submit(pending),
            KeyCode::BackTab => return self.open(self.selected().saturating_sub(1), pending),
            KeyCode::Tab => return self.open(self.selected() + 1, pending),
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

    /// Complains in the pane rather than on the error row, about a line that was typed or a call
    /// it asked for.
    ///
    /// The row is cleared by the next read that works, and a read working says nothing about
    /// either of those. As a notice it stays until the next line is typed.
    pub fn warn(&mut self, message: String) {
        self.notices = vec![ui::warning(&message)];
        self.scroll = None;
        self.changed();
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
    fn open(&mut self, index: usize, pending: bool) -> Option<Intent> {
        let index = index.min(self.rooms.len().saturating_sub(1));
        // Refused before the selection moves: watching the room is a call, and moving with none on
        // its way would leave the pane cleared and empty.
        if index == self.selected() || pending {
            return None;
        }

        self.open.select(Some(index));
        self.messages.clear();
        self.changed();

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

/// Marks every room read as it stands, so a fresh launch starts quiet rather than claiming the
/// whole history is new.
fn read_from(rooms: &[Room]) -> HashMap<RoomId, Seq> {
    rooms
        .iter()
        .map(|room| (room.id, room.latest_seq))
        .collect()
}
