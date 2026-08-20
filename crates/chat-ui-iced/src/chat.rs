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
//! The rooms, the open room's messages, and what is being typed.

use chat_client_core::v1::{Message as ChatMessage, Room, RoomId};
use iced::{
    Element, Length,
    widget::{button, column, container, row, rule, scrollable, text, text_input},
};

use crate::{app::Message, ui::error_line};

/// How wide the sidebar and the sender column are.
const SIDEBAR: f32 = 170.0;
const SENDER: f32 = 90.0;

/// The chat screen's state.
pub struct Chat {
    rooms: Vec<Room>,
    open: Option<RoomId>,
    /// The open room's messages, oldest first.
    messages: Vec<ChatMessage>,
    pub draft: String,
    pub error: Option<String>,
    /// Who is signed in, so their own lines can be told apart.
    username: String,
}

impl Chat {
    pub fn new(rooms: Vec<Room>, username: String) -> Self {
        Chat {
            rooms,
            open: None,
            messages: Vec::new(),
            draft: String::new(),
            error: None,
            username,
        }
    }

    /// The open room, which every call about messages needs.
    pub fn open_room(&self) -> Option<RoomId> {
        self.open
    }

    /// Switches rooms, dropping what the last one had shown.
    ///
    /// Per-room history is not kept: re-opening a room asks the server for its newest page again.
    pub fn open(&mut self, room: RoomId) {
        self.open = Some(room);
        self.messages.clear();
    }

    /// Appends a batch. They arrive oldest first, and the first one is the backfill.
    pub fn append(&mut self, messages: Vec<ChatMessage>) {
        self.messages.extend(messages);
    }

    fn open_name(&self) -> &str {
        self.rooms
            .iter()
            .find(|room| Some(room.id) == self.open)
            .map_or("", |room| room.name.as_str())
    }
}

pub fn view(state: &Chat) -> Element<'_, Message> {
    row![
        container(sidebar(state)).width(SIDEBAR),
        rule::vertical(1),
        column![
            container(text(format!("#{}", state.open_name())).size(20)).padding(12),
            rule::horizontal(1),
            messages(state),
            rule::horizontal(1),
            container(composer(state)).padding(12),
        ],
    ]
    .into()
}

fn sidebar(state: &Chat) -> Element<'_, Message> {
    let rooms = state.rooms.iter().map(|room| {
        let open = Some(room.id) == state.open;
        button(text(format!("#{}", room.name)))
            .on_press(Message::RoomOpened(room.id))
            // The open room is the only one that reads as pressed.
            .style(if open { button::primary } else { button::text })
            .width(Length::Fill)
            .into()
    });

    container(
        column![
            container(text("Rooms").size(14)).padding([12, 8]),
            column(rooms).spacing(2),
        ]
        .spacing(4),
    )
    .padding(4)
    .into()
}

fn messages(state: &Chat) -> Element<'_, Message> {
    let lines = state.messages.iter().map(|message| {
        let mine = message.username == state.username;
        row![
            text(&message.username)
                .width(SENDER)
                // Their own name, so a glance finds it.
                .style(if mine { text::primary } else { text::secondary }),
            text(&message.body),
        ]
        .spacing(8)
        .into()
    });

    // Anchored to the bottom, so the newest line is the one on screen.
    scrollable(container(column(lines).spacing(4)).padding(12))
        .anchor_bottom()
        .height(Length::Fill)
        .width(Length::Fill)
        .into()
}

fn composer(state: &Chat) -> Element<'_, Message> {
    let send = (!state.draft.trim().is_empty()).then_some(Message::Send);

    column![
        row![
            text_input("type a message…", &state.draft)
                .on_input(Message::DraftEdited)
                .on_submit(Message::Send)
                .padding(10),
            button(text("Send")).on_press_maybe(send).padding([8, 16]),
        ]
        .spacing(8),
        error_line(state.error.as_deref()),
    ]
    .spacing(6)
    .into()
}
