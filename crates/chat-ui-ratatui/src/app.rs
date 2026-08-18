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

use std::io;

use chat_client_core::{ChatClient, ChatError, ClientConfig, TransportKind, Url, v1::Message};
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures::StreamExt as _;
use ratatui::{DefaultTerminal, Frame};

use crate::{chat::Chat, connection::Connection, sign_in, sign_in::SignIn};

/// How many messages a room opens with.
const PAGE: usize = 50;

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
    exit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            screen: Screen::Connection(Connection::default()),
            client: None,
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

        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;

            if let Some(event) = keys.next().await {
                match event? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        self.handle_key(key).await;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();

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

    /// Creates the account and stays put. The user then logs in, as the API requires.
    async fn register(&mut self, username: &str, password: &str) {
        let Some(client) = self.client.clone() else {
            return;
        };

        if let Err(error) = client.register(username, password).await {
            self.failed(error);
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
                self.screen = Screen::Chat(Chat::new(rooms));
                self.open_room().await;
            }
            Err(error) => self.failed(error),
        }
    }

    /// Fills the message pane with the open room's newest page.
    async fn open_room(&mut self) {
        let (Some(client), Screen::Chat(screen)) = (self.client.clone(), &mut self.screen) else {
            return;
        };
        let Some(room) = screen.open_room().map(|room| room.id) else {
            return;
        };

        match client.messages_newest(room, PAGE).await {
            Ok(messages) => self.showing(messages),
            Err(error) => self.failed(error),
        }
    }

    /// Posts a message, then reads the room back so the sent line appears.
    ///
    /// A failed send puts the text back in the composer rather than losing it.
    async fn send(&mut self, body: String) {
        let (Some(client), Screen::Chat(screen)) = (self.client.clone(), &mut self.screen) else {
            return;
        };
        let Some(room) = screen.open_room().map(|room| room.id) else {
            return;
        };

        match client.send(room, &body).await {
            Ok(_) => self.open_room().await,
            Err(error) => {
                screen.restore(body);
                self.failed(error);
            }
        }
    }

    fn showing(&mut self, messages: Vec<Message>) {
        if let Screen::Chat(screen) = &mut self.screen {
            screen.show(messages);
        }
    }

    /// Shows the failure on the screen the user is on, and sends an ended session back to signing
    /// in — the one failure only the user can fix.
    fn failed(&mut self, error: ChatError) {
        let message = error.to_string();

        if matches!(error, ChatError::SessionExpired)
            && matches!(self.screen, Screen::Chat(_) | Screen::SignIn(_))
        {
            let mut screen = SignIn::default();
            screen.error = Some(message);
            self.screen = Screen::SignIn(screen);
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
