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

use std::{future::Future, io, time::Duration};

use chat_client_core::{
    ChatClient, ChatError, ClientConfig, MessagesFeed, PollConfig, RoomsFeed, Since, TransportKind,
    v1::{Message, Room},
};
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind};
use futures::{StreamExt as _, stream::BoxStream};
use ratatui::{DefaultTerminal, Frame, style::Style, widgets::Block};
use tokio::sync::mpsc;
use url::Url;

use crate::{
    CONTROL,
    screens::{
        chat::{self, Chat},
        connection::Connection,
        sign_in::{self, SignIn},
    },
    ui::theme,
};

/// How often the open room is re-read. The sidebar keeps the default, being less urgent.
const MESSAGES_REFRESH: Duration = Duration::from_secs(1);

/// The most answers that may wait to be read. One call is out at a time, so this is never reached;
/// it is a bound so that a bug grows a queue no further than this before it blocks.
const MAX_ANSWERS: usize = 8;

/// What woke the loop.
enum Woken {
    /// Something from the terminal, or `None` once it is closed.
    Terminal(Option<Event>),
    /// The open room's stream delivered a batch, or failed.
    Messages(Result<Vec<Message>, ChatError>),
    /// The sidebar's stream delivered a list, or failed.
    Rooms(Result<Vec<Room>, ChatError>),
    /// A call made away from the loop came back.
    Answer(Answer),
}

/// What a call made away from the loop came back with.
///
/// One variant per call the app makes. The fields beside a `result` are what the answer cannot be
/// acted on without: who logged in, or the text a failed send has to give back.
enum Answer {
    Connected(Result<ChatClient, ChatError>),
    Registered(Result<(), ChatError>),
    LoggedIn {
        username: String,
        result: Result<(RoomsFeed, Vec<Room>), ChatError>,
    },
    RoomOpened(Result<MessagesFeed, ChatError>),
    MessageSent {
        body: String,
        result: Result<(), ChatError>,
    },
    RoomCreated(Result<(), ChatError>),
}

/// The calls that run away from the loop, and how they come back.
///
/// Every call the user asks for goes through here, so the loop itself never waits on a server and
/// keeps drawing, reading keys and taking messages while one is out.
struct Background {
    answers: mpsc::Receiver<Answer>,
    /// The end a call answers through. Cloned into every one of them.
    answer_to: mpsc::Sender<Answer>,
    /// Whether a call the user asked for is still out. One at a time: a second Ctrl+R cannot make
    /// a second account, and two messages keep the order they were typed in.
    pending: bool,
}

impl Default for Background {
    fn default() -> Self {
        let (answer_to, answers) = mpsc::channel(MAX_ANSWERS);

        Self {
            answers,
            answer_to,
            pending: false,
        }
    }
}

impl Background {
    /// Starts a call, and answers whether it took it. A call already out is refused.
    ///
    /// Refusing quietly is safe for a caller that does nothing else, which is most of them. One
    /// that throws something away first has to ask before it does.
    fn ask(&mut self, work: impl Future<Output = Answer> + Send + 'static) -> bool {
        if self.pending {
            return false;
        }
        self.pending = true;

        let answer_to = self.answer_to.clone();
        tokio::spawn(async move {
            // The app is gone if this fails, and there is nobody left to tell.
            let _ = answer_to.send(work.await).await;
        });

        true
    }
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
    /// The open room's messages. One at a time: switching rooms drops this and opens another.
    ///
    /// A stream rather than the feed, and so is [`rooms`](Self::rooms), because a `select!` drops
    /// whichever arms did not win. A stream keeps a part-finished read inside itself, so a
    /// keypress costs a borrow rather than the request in flight.
    messages: Option<BoxStream<'static, Result<Vec<Message>, ChatError>>>,
    /// The sidebar's rooms, from the moment someone signs in.
    rooms: Option<BoxStream<'static, Result<Vec<Room>, ChatError>>>,
    /// Whether opening the room failed, so the next room list asks again.
    ///
    /// A failed open leaves nothing behind to start the polling: the arm that carries messages is
    /// held shut while there is no stream, and a room cannot be re-entered when it is the only
    /// one.
    reopen: bool,
    background: Background,
    exit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            screen: Screen::Connection(Connection::default()),
            client: None,
            messages: None,
            rooms: None,
            reopen: false,
            background: Background::default(),
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

            // Each arm produces what happened, so the borrows the select holds end before anything
            // acts on it.
            let woken = tokio::select! {
                event = keys.next() => Woken::Terminal(event.transpose()?),
                batch = next_messages(&mut self.messages), if self.messages.is_some() => Woken::Messages(batch),
                list = next_rooms(&mut self.rooms), if self.rooms.is_some() => Woken::Rooms(list),
                Some(answer) = self.background.answers.recv() => Woken::Answer(answer),
            };

            // Nothing here waits on a server: every call the user asks for is started by one of
            // these and comes back as `Woken::Answer`.
            match woken {
                Woken::Terminal(Some(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                    self.handle_key(key);
                }
                Woken::Terminal(None) => self.exit = true,
                Woken::Terminal(Some(_)) => {}
                Woken::Messages(batch) => self.apply_messages(batch),
                Woken::Rooms(list) => self.apply_rooms(list),
                Woken::Answer(answer) => self.answer(answer),
            }
        }
        Ok(())
    }

    /// Shows what the sidebar's stream delivered, or reports why it could not.
    fn apply_rooms(&mut self, list: Result<Vec<Room>, ChatError>) {
        match list {
            // Answering at all is the sign the server is back. Without this the warning from a
            // failed read outlives the failure, in a quiet room for as long as it stays quiet.
            Ok(rooms) => {
                if let Screen::Chat(screen) = &mut self.screen {
                    screen.show_rooms(rooms);
                    screen.error = None;
                }
                if self.reopen {
                    self.open_room();
                }
            }
            Err(error) => self.failed(error),
        }
    }

    /// Appends what the open room's stream delivered, or reports why it could not.
    fn apply_messages(&mut self, batch: Result<Vec<Message>, ChatError>) {
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

    fn handle_key(&mut self, key: KeyEvent) {
        // Raw mode hands Ctrl+C over as a key rather than a signal, so ending the app on it is this
        // loop's job. Claimed before any screen reads them, neither being a character to type.
        let quit = key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(CONTROL));
        if quit {
            self.exit = true;
            return;
        }
        if self.background.pending {
            return;
        }

        match &mut self.screen {
            Screen::Connection(screen) => {
                let Some(url) = screen.handle_key(key) else {
                    return;
                };
                self.connect(&url);
            }
            Screen::SignIn(screen) => {
                let Some(intent) = screen.handle_key(key) else {
                    return;
                };
                let (username, password) = screen.credentials();
                match intent {
                    sign_in::Intent::Register => self.register(&username, &password),
                    sign_in::Intent::LogIn => self.log_in(&username, &password),
                }
            }
            Screen::Chat(screen) => {
                let Some(intent) = screen.handle_key(key) else {
                    return;
                };
                match intent {
                    chat::Intent::Send(body) => self.send(body),
                    chat::Intent::Create(name) => self.create_room(&name),
                    chat::Intent::Open => self.open_room(),
                }
            }
        }
    }

    /// Acts on a call that came back, and starts the next one where a call leads to another.
    fn answer(&mut self, answer: Answer) {
        // Cleared first, so a call chained below is free to take its place.
        self.background.pending = false;

        match answer {
            Answer::Connected(Ok(client)) => {
                self.client = Some(client);
                self.screen = Screen::SignIn(SignIn::default());
            }
            // Read from the screen rather than carried here: it still holds them, no key having
            // been taken while the call was out, and a password is not something to put on a queue.
            Answer::Registered(Ok(())) => {
                if let Screen::SignIn(screen) = &self.screen {
                    let (username, password) = screen.credentials();
                    self.log_in(&username, &password);
                }
            }
            Answer::LoggedIn {
                username,
                result: Ok((rooms, listed)),
            } => {
                self.rooms = Some(rooms.into_stream().boxed());
                self.screen = Screen::Chat(Chat::new(listed, username));
                self.open_room();
            }
            Answer::RoomOpened(Ok(messages)) => {
                self.reopen = false;
                // Read before the feed becomes a stream, which takes it.
                let room = messages.room();
                if let Screen::Chat(screen) = &mut self.screen {
                    screen.watching(room);
                }
                self.messages = Some(messages.into_stream().boxed());
            }
            Answer::MessageSent {
                body,
                result: Err(error),
            } => {
                if let Screen::Chat(screen) = &mut self.screen {
                    screen.restore(body);
                }
                self.failed(error);
            }
            // Nothing to do: the sidebar reads on its own and picks the room up.
            Answer::RoomCreated(Ok(())) => {}
            // Nothing to do either: the message arrives on the feed like everyone else's.
            Answer::MessageSent { result: Ok(()), .. } => {}
            // Asked for again by the next room list, which is the only clock this screen has.
            Answer::RoomOpened(Err(error)) => {
                self.reopen = true;
                self.failed(error);
            }
            Answer::Connected(Err(error))
            | Answer::Registered(Err(error))
            | Answer::LoggedIn {
                result: Err(error), ..
            }
            | Answer::RoomCreated(Err(error)) => self.failed(error),
        }
    }

    /// Builds the client, then proves the server is there.
    ///
    /// Building it only parses configuration — nothing is dialled until a call is made — so the
    /// health check is what turns a wrong address into an error on this screen rather than a
    /// surprise on the next one.
    fn connect(&mut self, url: &str) {
        let url = url.to_owned();

        self.background.ask(async move {
            let built = async {
                let server_url =
                    Url::parse(&url).map_err(|error| ChatError::Config(error.to_string()))?;
                let client = ChatClient::new(ClientConfig {
                    transport: TransportKind::Tcp,
                    server_url,
                    poll: PollConfig {
                        messages_interval: MESSAGES_REFRESH,
                        ..PollConfig::default()
                    },
                    ..ClientConfig::default()
                })
                .await?;
                client.health().await?;

                Ok::<_, ChatError>(client)
            };

            Answer::Connected(built.await)
        });
    }

    /// Creates the account. Signing in follows when the answer comes back.
    ///
    /// The API keeps the two apart, so this is the screen composing them rather than the client
    /// doing it behind the caller's back.
    fn register(&mut self, username: &str, password: &str) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let (username, password) = (username.to_owned(), password.to_owned());

        self.background
            .ask(async move { Answer::Registered(client.register(&username, &password).await) });
    }

    /// Signs in and opens a feed on the rooms the server lists.
    fn log_in(&mut self, username: &str, password: &str) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let (username, password) = (username.to_owned(), password.to_owned());

        self.background.ask(async move {
            let result = async {
                client.login(&username, &password).await?;
                let mut rooms = client.watch_rooms().await?;
                // The feed hands over the list it opened with, so this costs no second call.
                let listed = rooms.next().await?;

                Ok::<_, ChatError>((rooms, listed))
            }
            .await;

            Answer::LoggedIn { username, result }
        });
    }

    /// Watches the open room, dropping whatever was being watched before.
    ///
    /// One at a time: the design keeps only the room on screen watched, and there is nothing to
    /// unsubscribe from — dropping the old one ends it.
    fn open_room(&mut self) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let Screen::Chat(screen) = &self.screen else {
            return;
        };
        let Some(room) = screen.open_room().map(|room| room.id) else {
            return;
        };

        // Asked for before the old room is let go of, so a refused call cannot leave a cleared
        // pane with nothing on its way to fill it.
        let asked = self.background.ask(async move {
            Answer::RoomOpened(client.watch_room_messages(room, Since::Newest).await)
        });
        if !asked {
            return;
        }

        if let Screen::Chat(screen) = &mut self.screen {
            screen.clear();
        }
        self.messages = None;
    }

    /// Posts a message.
    ///
    /// It is not shown here: it arrives on the feed like everyone else's, which is what keeps every
    /// client showing the same order. A failed send puts the text back rather than losing it.
    fn send(&mut self, body: String) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let Screen::Chat(screen) = &self.screen else {
            return;
        };
        let Some(room) = screen.open_room().map(|room| room.id) else {
            return;
        };

        self.background.ask(async move {
            let result = client.send(room, &body).await.map(drop);

            Answer::MessageSent { body, result }
        });
    }

    /// Creates a room and lets the sidebar pick it up.
    fn create_room(&mut self, name: &str) {
        let Some(client) = self.client.clone() else {
            return;
        };
        let name = name.to_owned();

        self.background
            .ask(async move { Answer::RoomCreated(client.create_room(&name).await.map(drop)) });
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
            self.messages = None;
            self.rooms = None;
            self.reopen = false;
            // Dropped with them, so an answer outliving the session is thrown away.
            self.background = Background::default();
            return;
        }

        match &mut self.screen {
            Screen::Connection(screen) => screen.error = Some(message),
            Screen::SignIn(screen) => screen.error = Some(message),
            Screen::Chat(screen) => screen.error = Some(message),
        }
    }
}

/// The open room's next batch. A free function, and so is [`next_rooms`], because one `select!`
/// waits on both and two methods would each borrow the whole app.
///
/// Only called with a stream open. Neither stream ever ends, so an end is read as the session
/// having gone, which is the one thing that would explain it.
async fn next_messages(
    messages: &mut Option<BoxStream<'static, Result<Vec<Message>, ChatError>>>,
) -> Result<Vec<Message>, ChatError> {
    match messages {
        Some(messages) => messages.next().await.unwrap_or(Err(ChatError::NotLoggedIn)),
        None => Err(ChatError::NotLoggedIn),
    }
}

/// The sidebar's next list. Only called with its stream open.
async fn next_rooms(
    rooms: &mut Option<BoxStream<'static, Result<Vec<Room>, ChatError>>>,
) -> Result<Vec<Room>, ChatError> {
    match rooms {
        Some(rooms) => rooms.next().await.unwrap_or(Err(ChatError::NotLoggedIn)),
        None => Err(ChatError::NotLoggedIn),
    }
}
