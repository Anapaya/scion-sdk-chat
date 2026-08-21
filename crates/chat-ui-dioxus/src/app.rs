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
//! The state every screen reads, and every call to the chat client.
//!
//! The screens render this state and hand back events; they never talk to a server. Everything that
//! does is in this file, so "where does this app use the SDK" has one answer.

use std::collections::HashMap;

use chat_client_core::{
    ChatClient, ChatError, ClientConfig, Since, TransportKind,
    v1::{Message as ChatMessage, Room, RoomId, Seq},
};
use dioxus::prelude::*;
use futures::StreamExt as _;

/// How often the sidebar is re-read.
///
/// The feed covers the open room's messages. The room list has no feed, and another client can add
/// a room at any time, so this app owns the timer that notices.
pub const ROOMS_REFRESH: std::time::Duration = std::time::Duration::from_secs(2);

/// The dev server, so the common case is one keypress.
const DEV_SERVER_URL: &str = "http://127.0.0.1:8080";

/// What typing this in the composer creates a room instead of sending a message.
const ROOM_COMMAND: &str = "/room";

/// Which screen is showing. The flow is one way, except that an ended session goes back to signing
/// in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Connection,
    SignIn,
    Chat,
}

/// What pressing Enter in the composer asks for.
pub enum Submission {
    /// Post this text to the open room.
    Send(String),
    /// Create a room with this name.
    Create(String),
}

/// Everything the screens read and write.
///
/// `Copy`, because a `Signal` is: the whole struct can be handed to an event handler or an async
/// task without cloning anything, which is what keeps the call sites below short.
#[derive(Clone, Copy)]
pub struct State {
    pub screen: Signal<Screen>,
    pub server_url: Signal<String>,
    pub username: Signal<String>,
    pub password: Signal<String>,
    pub draft: Signal<String>,
    pub error: Signal<Option<String>>,
    /// A call is out, so the button that started it is not pressable again.
    pub busy: Signal<bool>,

    pub rooms: Signal<Vec<Room>>,
    pub open: Signal<Option<RoomId>>,
    /// The open room's messages, oldest first.
    pub messages: Signal<Vec<ChatMessage>>,
    /// The newest `seq` the user has actually seen in each room. Only the room on screen advances
    /// it, which is what makes it the badge's cursor rather than a resume cursor.
    last_read: Signal<HashMap<RoomId, Seq>>,

    client: Signal<Option<ChatClient>>,
}

impl State {
    /// The state a launch starts from, put in context so every screen reaches the same one.
    pub fn new() -> Self {
        State {
            screen: Signal::new(Screen::Connection),
            server_url: Signal::new(DEV_SERVER_URL.to_owned()),
            username: Signal::new(String::new()),
            password: Signal::new(String::new()),
            draft: Signal::new(String::new()),
            error: Signal::new(None),
            busy: Signal::new(false),
            rooms: Signal::new(Vec::new()),
            open: Signal::new(None),
            messages: Signal::new(Vec::new()),
            last_read: Signal::new(HashMap::new()),
            client: Signal::new(None),
        }
    }

    /// Builds the client, then proves the server is there.
    ///
    /// Building it only parses configuration — nothing is dialled until a call is made — so the
    /// health check is what turns a wrong address into an error on the connection screen rather
    /// than a surprise on the next one.
    pub async fn connect(mut self) {
        let url = self.server_url.peek().clone();
        self.busy.set(true);
        self.error.set(None);

        let built = async {
            let server_url =
                url::Url::parse(&url).map_err(|error| ChatError::Config(error.to_string()))?;
            let client = ChatClient::new(ClientConfig {
                transport: TransportKind::Tcp,
                server_url,
                ..ClientConfig::default()
            })
            .await?;
            client.health().await?;

            Ok::<_, ChatError>(client)
        };

        match built.await {
            Ok(client) => {
                self.client.set(Some(client));
                self.screen.set(Screen::SignIn);
            }
            Err(error) => {
                self.failed(error);
            }
        }
        self.busy.set(false);
    }

    /// Creates the account, then signs in with it.
    ///
    /// The API keeps the two apart, so this is the app composing them rather than the client doing
    /// it behind the caller's back.
    pub async fn register(mut self) {
        let Some(client) = self.client() else { return };
        let (username, password) = self.credentials();
        self.busy.set(true);
        self.error.set(None);

        match client.register(&username, &password).await {
            Ok(()) => {
                self.busy.set(false);
                self.log_in().await;
            }
            Err(error) => {
                self.failed(error);
                self.busy.set(false);
            }
        }
    }

    /// Signs in and opens the chat over the rooms the server lists.
    pub async fn log_in(mut self) {
        let Some(client) = self.client() else { return };
        let (username, password) = self.credentials();
        self.busy.set(true);
        self.error.set(None);

        let opened = async {
            client.login(&username, &password).await?;

            client.rooms().await
        };

        match opened.await {
            Ok(rooms) => {
                // Every room is marked read as it stands, so a launch starts quiet rather than
                // claiming the whole history is new.
                self.last_read.set(
                    rooms
                        .iter()
                        .map(|room| (room.id, room.latest_seq))
                        .collect(),
                );
                // Lobby always exists, so it is what the screen opens with.
                let opening = rooms
                    .iter()
                    .find(|room| room.name.eq_ignore_ascii_case("lobby"))
                    .or_else(|| rooms.first())
                    .map(|room| room.id);

                self.rooms.set(rooms);
                self.screen.set(Screen::Chat);
                if let Some(room) = opening {
                    self.open_room(room);
                }
            }
            Err(error) => {
                self.failed(error);
            }
        }
        self.busy.set(false);
    }

    /// Switches rooms, dropping what the last one had shown.
    ///
    /// Per-room history is not kept: re-opening a room asks the server for its newest page again.
    /// Writing `open` is all this does — the feed watches that signal and restarts itself.
    pub fn open_room(&mut self, room: RoomId) {
        self.messages.write().clear();
        self.open.set(Some(room));
    }

    /// The open room's messages, until the room changes.
    ///
    /// Runs as one long future rather than a timer: [`RoomFeed`] owns the cadence and the cursor,
    /// and a client that wrote its own timer would be duplicating both.
    ///
    /// [`RoomFeed`]: chat_client_core::RoomFeed
    pub async fn watch(mut self, room: RoomId) {
        let Some(client) = self.client() else { return };

        let feed = match client.watch_room(room, Since::Newest).await {
            Ok(feed) => feed,
            Err(error) => {
                self.failed(error);
                return;
            }
        };
        let mut batches = std::pin::pin!(feed.into_stream());

        while let Some(batch) = batches.next().await {
            match batch {
                // Arriving at all is the sign the server is answering again.
                Ok(messages) => {
                    if let Some(newest) = messages.last().map(|message| message.seq) {
                        self.last_read.write().insert(room, newest);
                    }
                    self.messages.write().extend(messages);
                    self.error.set(None);
                }
                Err(error) => {
                    let ended = self.failed(error);
                    if ended {
                        return;
                    }
                }
            }
        }
    }

    /// Re-reads the room list.
    pub async fn refresh_rooms(mut self) {
        let Some(client) = self.client() else { return };

        match client.rooms().await {
            Ok(rooms) => self.rooms.set(rooms),
            Err(error) => {
                self.failed(error);
            }
        }
    }

    /// Sends what was typed, or creates the room it names.
    ///
    /// A posted message is not shown here: it arrives on the feed like everyone else's, which is
    /// what keeps every client showing the same order. The draft stays until the call succeeds.
    pub async fn submit(mut self) {
        let Some(client) = self.client() else { return };
        let Some(submission) = self.submission() else {
            return;
        };
        self.error.set(None);

        match submission {
            Submission::Send(body) => {
                let Some(room) = *self.open.peek() else {
                    return;
                };

                match client.send(room, &body).await {
                    Ok(_) => self.draft.set(String::new()),
                    Err(error) => {
                        self.failed(error);
                    }
                }
            }
            // `create_room` answers with the existing room when the name is taken, so a name
            // already in use lands here rather than as an error, and opens that room.
            Submission::Create(name) => {
                match client.create_room(&name).await {
                    Ok(room) => {
                        let opening = room.id;
                        if !self.rooms.peek().iter().any(|known| known.id == opening) {
                            self.rooms.write().push(room);
                        }
                        // The command is consumed, the same as a message that was posted.
                        self.draft.set(String::new());
                        self.open_room(opening);
                    }
                    Err(error) => {
                        self.failed(error);
                    }
                }
            }
        }
    }

    /// What pressing Enter would do, or `None` when there is nothing to do.
    pub fn submission(&self) -> Option<Submission> {
        let draft = self.draft.peek();
        let draft = draft.trim();

        if let Some(rest) = draft.strip_prefix(ROOM_COMMAND) {
            // Only with a separator, so `/roominfo` stays an ordinary message.
            if rest.is_empty() || rest.starts_with(' ') {
                let name = rest.trim();
                return nameable(name).then(|| Submission::Create(name.to_owned()));
            }
        }

        (!draft.is_empty()).then(|| Submission::Send(draft.to_owned()))
    }

    /// Whether a room holds anything the user has not seen.
    ///
    /// A yes or no, never a count: `seq` is assigned server-wide, so the gap between two of them
    /// counts messages posted to every other room as well. A real count needs that room's messages.
    pub fn unread(&self, room: &Room) -> bool {
        if *self.open.read() == Some(room.id) {
            return false;
        }

        room.latest_seq
            > self
                .last_read
                .read()
                .get(&room.id)
                .copied()
                .unwrap_or(Seq::START)
    }

    /// The name of the open room, for the header.
    pub fn open_name(&self) -> String {
        let open = *self.open.read();

        self.rooms
            .read()
            .iter()
            .find(|room| Some(room.id) == open)
            .map_or_else(String::new, |room| room.name.clone())
    }

    /// Shows the failure on the screen the user is on, and sends an ended session back to signing
    /// in — the one failure only the user can fix.
    ///
    /// Returns whether the session ended, which is what tells the feed to stop rather than keep
    /// asking with a token the server has refused.
    fn failed(&mut self, error: ChatError) -> bool {
        // One means the server refused the token, the other that the client already forgot it.
        // Retrying either only repeats it.
        let signed_out = matches!(error, ChatError::SessionExpired | ChatError::NotLoggedIn);
        self.error.set(Some(error.to_string()));

        if signed_out && *self.screen.peek() != Screen::Connection {
            self.screen.set(Screen::SignIn);
            self.open.set(None);
            self.messages.write().clear();
        }

        signed_out
    }

    fn client(&self) -> Option<ChatClient> {
        self.client.peek().clone()
    }

    fn credentials(&self) -> (String, String) {
        (self.username.peek().clone(), self.password.peek().clone())
    }
}

/// The same rule the server applies: 1 to 64 printable ASCII characters.
///
/// Checked here so a name the server would refuse leaves the button disabled rather than costing a
/// round trip.
fn nameable(name: &str) -> bool {
    (1..=64).contains(&name.len()) && name.chars().all(|c| c.is_ascii_graphic() || c == ' ')
}
