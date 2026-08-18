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

use chat_client_core::v1::{Message, Room};
use crossterm::event::{Event, KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Stylize},
    text::{Line, Span},
    widgets::{Block, List, ListState, Paragraph},
};
use tui_input::{Input, backend::crossterm::EventHandler as _};

use crate::field;

/// How wide the room list is. Fixed, because a room name is short and the messages want the rest.
const SIDEBAR_WIDTH: u16 = 18;

/// How much room a name is given before the message it sent.
const NAME_WIDTH: usize = 10;

/// What the screen is asking for.
pub enum Intent {
    /// Post what was typed to the open room.
    Send(String),
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
    input: Input,
    /// What went wrong last time. Shown until it recovers.
    pub error: Option<String>,
}

impl Chat {
    /// Opens over the rooms the server listed, with the first one — always the lobby — selected.
    pub fn new(rooms: Vec<Room>) -> Self {
        Self {
            rooms,
            open: ListState::default().with_selected(Some(0)),
            messages: Vec::new(),
            input: Input::default(),
            error: None,
        }
    }

    /// The room the sidebar has selected, if the server listed any.
    pub fn open_room(&self) -> Option<&Room> {
        self.rooms.get(self.selected())
    }

    /// Replaces what is on screen with the page just fetched.
    pub fn show(&mut self, messages: Vec<Message>) {
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
        field::draw(frame, composer, "", &self.input, true, false);
        if let Some(message) = &self.error {
            frame.render_widget(Paragraph::new(format!("⚠ {message}")).red(), error);
        }
    }

    fn draw_rooms(&mut self, frame: &mut Frame, area: Rect) {
        let names = self.rooms.iter().map(|room| room.name.as_str());
        let list = List::new(names)
            .block(Block::bordered().title(" Rooms ".bold()))
            .highlight_symbol("▸ ")
            .highlight_style(Modifier::BOLD);

        frame.render_stateful_widget(list, area, &mut self.open);
    }

    fn draw_messages(&self, frame: &mut Frame, area: Rect) {
        let title = match self.open_room() {
            Some(room) => format!(" #{} ", room.name),
            None => " no rooms ".to_owned(),
        };
        let lines = self.messages.iter().map(|message| {
            Line::from(vec![
                Span::from(format!("{:<NAME_WIDTH$}", message.username)).blue(),
                Span::from(message.body.as_str()),
            ])
        });

        frame.render_widget(
            List::new(lines).block(Block::bordered().title(title.bold())),
            area,
        );
    }

    /// Claims the keys that mean something here and leaves the rest to the input.
    ///
    /// Up and Down are free for the sidebar because the composer is one line and does not read
    /// them.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Intent> {
        match key.code {
            KeyCode::Enter => return self.send(),
            KeyCode::Up => return self.open(self.selected().saturating_sub(1)),
            KeyCode::Down => return self.open(self.selected() + 1),
            _ => {
                self.input.handle_event(&Event::Key(key));
            }
        }
        None
    }

    /// Takes what was typed, leaving the line empty. A blank line sends nothing.
    fn send(&mut self) -> Option<Intent> {
        let body = self.input.value_and_reset();

        (!body.trim().is_empty()).then_some(Intent::Send(body))
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

    fn selected(&self) -> usize {
        self.open.selected().unwrap_or(0)
    }
}
