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
//! Which screen is showing, and every call to the chat client.
//!
//! The screens read keys and draw; they never talk to a server. Everything that does is in this
//! file, so "where does this app use the SDK" has one answer.

use std::{io, time::Duration};

use chat_client_core::{
    ChatClient, ChatError, ClientConfig, RoomFeed, Since, TransportKind, Url, v1::Message,
};
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt as _;
use ratatui::{DefaultTerminal, Frame, style::Style, widgets::Block};

use crate::{chat::Chat, connection::Connection, sign_in, sign_in::SignIn, theme};

/// How often the sidebar is re-read.
///
/// The feed covers the open room's messages. The room list has no feed, so a client that wants it
/// fresh owns this timer.
const ROOMS_REFRESH: Duration = Duration::from_secs(2);

/// What woke the loop.
enum Woken {
    /// Something from the terminal, or `None` once it is closed.
    Terminal(Option<Event>),
    /// The feed delivered a batch, or failed.
    Feed(Result<Vec<Message>, ChatError>),
    /// The sidebar is due a re-read.
    Rooms,
}

/// Which screen is showing. The flow is one way, except that an ended session goes back to signing
/// in.
enum Screen {
    Connection(Connection),
    SignIn(SignIn),
    Chat(Chat),
}

/// The whole app: a screen, and the client once one has been built.
pub struct App {
    screen: Screen,
    /// Built on the connection screen. Cloning is cheap and shares the session.
    client: Option<ChatClient>,
    /// The open room's feed. One at a time: switching rooms drops this and opens another.
    feed: Option<RoomFeed>,
    exit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            screen: Screen::Connection(Connection::default()),
            client: None,
            feed: None,
            exit: false,
        }
    }
}

impl App {
    /// Draws, then waits for a key, until asked to stop.
    ///
    /// Keys arrive as a stream rather than a blocking read, so that a `select!` can wait on the
    /// terminal and on the network at the same time.
    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let mut keys = EventStream::new();
        let mut rooms = tokio::time::interval(ROOMS_REFRESH);

        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;

            // Each arm produces what happened, so the borrows the select holds end before anything
            // acts on it.
            let woken = tokio::select! {
                event = keys.next() => Woken::Terminal(event.transpose()?),
                batch = self.next_batch(), if self.feed.is_some() => Woken::Feed(batch),
                _ = rooms.tick() => Woken::Rooms,
            };

            match woken {
                Woken::Terminal(Some(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                    self.handle_key(key).await;
                }
                Woken::Terminal(None) => self.exit = true,
                Woken::Terminal(Some(_)) => {}
                Woken::Feed(batch) => self.apply(batch),
                Woken::Rooms => self.refresh_rooms().await,
            }
        }
        Ok(())
    }

    /// The feed's next batch. Only called with a feed open.
    async fn next_batch(&mut self) -> Result<Vec<Message>, ChatError> {
        match &mut self.feed {
            Some(feed) => feed.next().await,
            None => Err(ChatError::NotLoggedIn),
        }
    }

    /// Appends what the feed delivered, or reports why it could not.
    fn apply(&mut self, batch: Result<Vec<Message>, ChatError>) {
        match batch {
            // Arriving at all is the sign the server is answering again.
            Ok(messages) => {
                if let Screen::Chat(screen) = &mut self.screen {
                    screen.append(messages);
                    screen.error = None;
                }
            }
            Err(error) => self.failed(error),
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        frame.render_widget(Block::new().style(Style::new().bg(theme::BACKGROUND)), area);

        match &mut self.screen {
            Screen::Connection(screen) => screen.draw(frame, area),
            Screen::SignIn(screen) => screen.draw(frame, area),
            Screen::Chat(screen) => screen.draw(frame, area),
        }
    }

    async fn handle_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Esc {
            self.exit = true;
            return;
        }

        match &mut self.screen {
            Screen::Connection(screen) => {
                let Some(url) = screen.handle_key(key) else {
                    return;
                };
                self.connect(&url).await;
            }
            Screen::SignIn(screen) => {
                let Some(intent) = screen.handle_key(key) else {
                    return;
                };
                let (username, password) = screen.credentials();
                match intent {
                    sign_in::Intent::Register => self.register(&username, &password).await,
                    sign_in::Intent::LogIn => self.log_in(&username, &password).await,
                }
            }
            Screen::Chat(screen) => {
                let Some(intent) = screen.handle_key(key) else {
                    return;
                };
                match intent {
                    crate::chat::Intent::Send(body) => self.send(body).await,
                    crate::chat::Intent::Create(name) => self.create_room(&name).await,
                    crate::chat::Intent::Open => self.open_room().await,
                }
            }
        }
    }

    /// Builds the client, then proves the server is there.
    ///
    /// Building it only parses configuration — nothing is dialled until a call is made — so the
    /// health check is what turns a wrong address into an error on this screen rather than a
    /// surprise on the next one.
    async fn connect(&mut self, url: &str) {
        let built = async {
            let server_url =
                Url::parse(url).map_err(|error| ChatError::Config(error.to_string()))?;
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
                self.client = Some(client);
                self.screen = Screen::SignIn(SignIn::default());
            }
            Err(error) => self.failed(error),
        }
    }

    /// Creates the account, then signs in with it.
    ///
    /// The API keeps the two apart, so this is the screen composing them rather than the client
    /// doing it behind the caller's back.
    async fn register(&mut self, username: &str, password: &str) {
        let Some(client) = self.client.clone() else {
            return;
        };

        match client.register(username, password).await {
            Ok(()) => self.log_in(username, password).await,
            Err(error) => self.failed(error),
        }
    }

    /// Signs in and opens the chat over the rooms the server lists.
    async fn log_in(&mut self, username: &str, password: &str) {
        let Some(client) = self.client.clone() else {
            return;
        };

        let opened = async {
            client.login(username, password).await?;

            client.rooms().await
        };

        match opened.await {
            Ok(rooms) => {
                self.screen = Screen::Chat(Chat::new(rooms, username.to_owned()));
                self.open_room().await;
            }
            Err(error) => self.failed(error),
        }
    }

    /// Watches the open room, dropping whatever was being watched before.
    ///
    /// One feed at a time: the design keeps only the room on screen watched, and there is nothing
    /// to unsubscribe from — dropping the old one ends it.
    async fn open_room(&mut self) {
        let (Some(client), Screen::Chat(screen)) = (self.client.clone(), &mut self.screen) else {
            return;
        };
        let Some(room) = screen.open_room().map(|room| room.id) else {
            return;
        };
        screen.clear();
        self.feed = None;

        match client.watch_room(room, Since::Newest).await {
            Ok(feed) => {
                if let Screen::Chat(screen) = &mut self.screen {
                    screen.watching(feed.room());
                }
                self.feed = Some(feed);
            }
            Err(error) => self.failed(error),
        }
    }

    /// Posts a message.
    ///
    /// It is not shown here: it arrives on the feed like everyone else's, which is what keeps every
    /// client showing the same order. A failed send puts the text back rather than losing it.
    async fn send(&mut self, body: String) {
        let (Some(client), Screen::Chat(screen)) = (self.client.clone(), &mut self.screen) else {
            return;
        };
        let Some(room) = screen.open_room().map(|room| room.id) else {
            return;
        };

        if let Err(error) = client.send(room, &body).await {
            screen.restore(body);
            self.failed(error);
        }
    }

    /// Creates a room and lets the sidebar pick it up.
    async fn create_room(&mut self, name: &str) {
        let Some(client) = self.client.clone() else {
            return;
        };

        match client.create_room(name).await {
            Ok(_) => self.refresh_rooms().await,
            Err(error) => self.failed(error),
        }
    }

    /// Re-reads the sidebar. Does nothing on the other screens.
    async fn refresh_rooms(&mut self) {
        let (Some(client), Screen::Chat(_)) = (self.client.clone(), &self.screen) else {
            return;
        };

        match client.rooms().await {
            Ok(rooms) => {
                if let Screen::Chat(screen) = &mut self.screen {
                    screen.show_rooms(rooms);
                }
            }
            Err(error) => self.failed(error),
        }
    }

    /// Shows the failure on the screen the user is on, and sends an ended session back to signing
    /// in — the one failure only the user can fix.
    fn failed(&mut self, error: ChatError) {
        let message = error.to_string();

        // Both mean the session is gone: one because the server refused the token, the other
        // because the client already forgot it. Retrying either only repeats it.
        let signed_out = matches!(error, ChatError::SessionExpired | ChatError::NotLoggedIn);

        if signed_out && matches!(self.screen, Screen::Chat(_) | Screen::SignIn(_)) {
            let mut screen = SignIn::default();
            screen.error = Some(message);
            self.screen = Screen::SignIn(screen);
            self.feed = None;
            return;
        }

        match &mut self.screen {
            Screen::Connection(screen) => screen.error = Some(message),
            Screen::SignIn(screen) => screen.error = Some(message),
            Screen::Chat(screen) => screen.error = Some(message),
        }
    }
}

/// The modifier a screen checks for, named once so the screens do not import crossterm's whole
/// keyboard.
pub const CONTROL: KeyModifiers = KeyModifiers::CONTROL;
