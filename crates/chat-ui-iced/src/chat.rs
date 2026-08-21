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

use std::collections::HashMap;

use chat_client_core::v1::{Message as ChatMessage, Room, RoomId, Seq};
use iced::{
    Element, Font, Length,
    font::Weight,
    widget::{button, column, container, row, rule, scrollable, text, text_input},
};

use crate::{app::Message, ui::error_line};

/// How wide the sidebar and the sender column are.
const SIDEBAR: f32 = 170.0;
const SENDER: f32 = 90.0;

/// What typing this in the composer creates a room instead of sending a message.
const ROOM_COMMAND: &str = "/room";

/// What an unread room's name is drawn in, next to everyone else's.
const BOLD: Font = Font {
    weight: Weight::Bold,
    ..Font::DEFAULT
};

/// What the composer is asking for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Submission {
    /// Post this text to the open room.
    Send(String),
    /// Create a room with this name.
    Create(String),
}

/// The chat screen's state.
pub struct Chat {
    rooms: Vec<Room>,
    open: Option<RoomId>,
    /// The open room's messages, oldest first.
    messages: Vec<ChatMessage>,
    /// The newest `seq` the user has actually seen in each room. Only the room on screen advances
    /// it, which is what makes it the badge's cursor rather than a resume cursor.
    last_read: HashMap<RoomId, Seq>,
    pub draft: String,
    pub error: Option<String>,
    /// Who is signed in, so their own lines can be told apart.
    username: String,
}

impl Chat {
    pub fn new(rooms: Vec<Room>, username: String) -> Self {
        Chat {
            last_read: read_from(&rooms),
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

    /// Appends a batch, and marks the open room read up to it.
    ///
    /// They arrive oldest first and never overlap, so appending is all there is to do.
    pub fn append(&mut self, messages: Vec<ChatMessage>) {
        if let (Some(room), Some(newest)) = (self.open, messages.last().map(|last| last.seq)) {
            self.last_read.insert(room, newest);
        }
        self.messages.extend(messages);
    }

    /// Whether a room holds anything the user has not seen.
    ///
    /// A yes or no, never a count: `seq` is assigned server-wide, so the gap between two of them
    /// counts messages posted to every other room as well. A real count needs that room's messages.
    fn unread(&self, room: &Room) -> bool {
        if self.open == Some(room.id) {
            return false;
        }

        room.latest_seq > self.last_read.get(&room.id).copied().unwrap_or(Seq::START)
    }

    /// Replaces the room list, keeping whichever room is open.
    ///
    /// Rooms are never deleted, so the open room is always still in the new list.
    pub fn show_rooms(&mut self, rooms: Vec<Room>) {
        self.rooms = rooms;
    }

    /// Adds a room, or leaves it alone if the list already has it.
    ///
    /// `create_room` answers with the existing room when the name is taken, so this is the same
    /// call whether the room was just made or was already there.
    pub fn add_room(&mut self, room: Room) {
        if !self.rooms.iter().any(|known| known.id == room.id) {
            self.rooms.push(room);
        }
    }

    /// What pressing Enter would do, or `None` when there is nothing to do.
    pub fn submission(&self) -> Option<Submission> {
        let draft = self.draft.trim();

        if let Some(rest) = draft.strip_prefix(ROOM_COMMAND) {
            // Only with a separator, so `/roominfo` stays an ordinary message.
            if rest.is_empty() || rest.starts_with(' ') {
                let name = rest.trim();
                return nameable(name).then(|| Submission::Create(name.to_owned()));
            }
        }

        (!draft.is_empty()).then(|| Submission::Send(draft.to_owned()))
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
        let unread = state.unread(room);

        // A dot rather than a number, for the reason `unread` gives.
        let name = text(if unread {
            format!("#{} ●", room.name)
        } else {
            format!("#{}", room.name)
        })
        .font(if unread { BOLD } else { Font::DEFAULT })
        // The open room keeps the filled button's own foreground; dimming it there would put low
        // contrast on a solid background.
        .style(if open {
            text::default
        } else if unread {
            text::primary
        } else {
            text::secondary
        });

        button(name)
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
    let submission = state.submission();
    // Naming the action is what tells the reader that Enter is about to do something other than
    // post what they typed.
    let label = match submission {
        Some(Submission::Create(_)) => "Create",
        _ => "Send",
    };
    let submit = submission.map(|_| Message::Send);

    column![
        row![
            text_input("type a message, or /room name to create one", &state.draft)
                .on_input(Message::DraftEdited)
                .on_submit(Message::Send)
                .padding(10),
            button(text(label)).on_press_maybe(submit).padding([8, 16]),
        ]
        .spacing(8),
        error_line(state.error.as_deref()),
    ]
    .spacing(6)
    .into()
}

/// Marks every room read as it stands, so a fresh launch starts quiet rather than claiming the
/// whole history is new.
fn read_from(rooms: &[Room]) -> HashMap<RoomId, Seq> {
    rooms
        .iter()
        .map(|room| (room.id, room.latest_seq))
        .collect()
}

/// The same rule the server applies: 1 to 64 printable ASCII characters.
///
/// Checked here so a name the server would refuse leaves the button disabled rather than costing a
/// round trip.
fn nameable(name: &str) -> bool {
    (1..=64).contains(&name.len()) && name.chars().all(|c| c.is_ascii_graphic() || c == ' ')
}
