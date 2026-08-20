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
//! The screens hold their own fields and draw them; they never talk to a server. Everything that
//! does is in this file, so "where does this app use the SDK" has one answer.

use chat_client_core::{
    ChatClient, ChatError, ClientConfig, Since, TransportKind,
    v1::{Message as ChatMessage, Room, RoomId},
};
use futures::{Stream, StreamExt as _, stream};
use iced::{Element, Task, Theme, task};
use url::Url;

use crate::{chat::Chat, connection::Connection, sign_in::SignIn};

/// A failure, flattened to what a screen can show.
///
/// iced needs a `Message` that clones, and `ChatError` does not, so the error cannot travel as
/// itself. The one distinction the app acts on is kept.
#[derive(Debug, Clone)]
pub struct Failure {
    pub text: String,
    /// The session is gone, so the only way on is to sign in again.
    signed_out: bool,
}

impl From<ChatError> for Failure {
    fn from(error: ChatError) -> Self {
        Failure {
            // One means the server refused the token, the other that the client already forgot it.
            // Retrying either only repeats it.
            signed_out: matches!(error, ChatError::SessionExpired | ChatError::NotLoggedIn),
            text: error.to_string(),
        }
    }
}

/// Everything the app reacts to.
///
/// No `Debug`: `ChatClient` does not derive it, and the client has to travel in `Connected` because
/// building one is asynchronous.
#[derive(Clone)]
pub enum Message {
    UrlEdited(String),
    Connect,
    Connected(Result<ChatClient, Failure>),
    UsernameEdited(String),
    PasswordEdited(String),
    Register,
    Registered(Result<(), Failure>),
    LogIn,
    LoggedIn(Result<Vec<Room>, Failure>),
    RoomOpened(RoomId),
    /// A batch from the open room's feed, oldest first.
    Batch(Result<Vec<ChatMessage>, Failure>),
    DraftEdited(String),
    Send,
    Sent(Result<(), Failure>),
}

/// Which screen is showing. The flow is one way, except that an ended session goes back to signing
/// in.
enum Screen {
    Connection(Connection),
    SignIn(SignIn),
    Chat(Chat),
}

/// The whole app: a screen, the client once one has been built, and the open room's feed.
#[derive(Default)]
pub struct App {
    screen: Screen,
    /// Cloning is cheap and shares the session.
    client: Option<ChatClient>,
    /// Aborts the open room's feed when it is dropped, which is what switching rooms does to it.
    feed: Option<task::Handle>,
}

impl Default for Screen {
    fn default() -> Self {
        Screen::Connection(Connection::default())
    }
}

impl App {
    pub fn theme(&self) -> Theme {
        Theme::CatppuccinMacchiato
    }

    pub fn view(&self) -> Element<'_, Message> {
        match &self.screen {
            Screen::Connection(screen) => crate::connection::view(screen),
            Screen::SignIn(screen) => crate::sign_in::view(screen),
            Screen::Chat(screen) => crate::chat::view(screen),
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::UrlEdited(url) => {
                if let Screen::Connection(screen) = &mut self.screen {
                    screen.url = url;
                }
                Task::none()
            }

            Message::Connect => {
                let Screen::Connection(screen) = &mut self.screen else {
                    return Task::none();
                };
                let url = screen.url.clone();
                screen.error = None;
                screen.busy = true;
                Task::perform(connect(url), Message::Connected)
            }

            Message::Connected(Ok(client)) => {
                self.client = Some(client);
                self.screen = Screen::SignIn(SignIn::default());
                Task::none()
            }

            Message::UsernameEdited(username) => {
                if let Screen::SignIn(screen) = &mut self.screen {
                    screen.username = username;
                }
                Task::none()
            }

            Message::PasswordEdited(password) => {
                if let Screen::SignIn(screen) = &mut self.screen {
                    screen.password = password;
                }
                Task::none()
            }

            Message::Register => {
                let (Some(client), Screen::SignIn(screen)) =
                    (self.client.clone(), &mut self.screen)
                else {
                    return Task::none();
                };
                let (username, password) = (screen.username.clone(), screen.password.clone());
                screen.error = None;
                screen.busy = true;
                Task::perform(
                    async move {
                        client
                            .register(&username, &password)
                            .await
                            .map_err(Failure::from)
                    },
                    Message::Registered,
                )
            }

            // The API keeps the two apart, so this is the app composing them rather than the client
            // doing it behind the caller's back.
            Message::Registered(Ok(())) => self.update(Message::LogIn),

            Message::LogIn => {
                let (Some(client), Screen::SignIn(screen)) =
                    (self.client.clone(), &mut self.screen)
                else {
                    return Task::none();
                };
                let (username, password) = (screen.username.clone(), screen.password.clone());
                screen.error = None;
                screen.busy = true;
                Task::perform(log_in(client, username, password), Message::LoggedIn)
            }

            Message::LoggedIn(Ok(rooms)) => {
                let username = match &self.screen {
                    Screen::SignIn(screen) => screen.username.clone(),
                    _ => String::new(),
                };
                // Lobby always exists, so it is what the screen opens with.
                let opening = rooms
                    .iter()
                    .find(|room| room.name.eq_ignore_ascii_case("lobby"))
                    .or_else(|| rooms.first())
                    .map(|room| room.id);
                self.screen = Screen::Chat(Chat::new(rooms, username));

                match opening {
                    Some(room) => self.update(Message::RoomOpened(room)),
                    None => Task::none(),
                }
            }

            Message::RoomOpened(room) => {
                let (Some(client), Screen::Chat(screen)) = (self.client.clone(), &mut self.screen)
                else {
                    return Task::none();
                };
                screen.open(room);

                let (task, handle) = Task::run(watch(client, room), Message::Batch).abortable();
                // Storing this drops the room's predecessor, which is what ends its feed. There is
                // nothing to unsubscribe from: one feed at a time, and the old one is simply let
                // go.
                self.feed = Some(handle.abort_on_drop());
                task
            }

            // Arriving at all is the sign the server is answering again.
            Message::Batch(Ok(messages)) => {
                if let Screen::Chat(screen) = &mut self.screen {
                    screen.append(messages);
                    screen.error = None;
                }
                Task::none()
            }

            Message::DraftEdited(draft) => {
                if let Screen::Chat(screen) = &mut self.screen {
                    screen.draft = draft;
                }
                Task::none()
            }

            // The message is not shown here: it arrives on the feed like everyone else's, which is
            // what keeps every client showing the same order. The draft stays until that succeeds.
            Message::Send => {
                let (Some(client), Screen::Chat(screen)) = (self.client.clone(), &mut self.screen)
                else {
                    return Task::none();
                };
                let (Some(room), body) = (screen.open_room(), screen.draft.trim().to_owned())
                else {
                    return Task::none();
                };
                if body.is_empty() {
                    return Task::none();
                }
                screen.error = None;

                Task::perform(
                    async move {
                        client
                            .send(room, &body)
                            .await
                            .map(|_| ())
                            .map_err(Failure::from)
                    },
                    Message::Sent,
                )
            }

            Message::Sent(Ok(())) => {
                if let Screen::Chat(screen) = &mut self.screen {
                    screen.draft.clear();
                }
                Task::none()
            }

            Message::Connected(Err(failure))
            | Message::Registered(Err(failure))
            | Message::LoggedIn(Err(failure))
            | Message::Batch(Err(failure))
            | Message::Sent(Err(failure)) => self.failed(failure),
        }
    }

    /// Shows the failure on the screen the user is on, and sends an ended session back to signing
    /// in — the one failure only the user can fix.
    fn failed(&mut self, failure: Failure) -> Task<Message> {
        if failure.signed_out && !matches!(self.screen, Screen::Connection(_)) {
            self.screen = Screen::SignIn(SignIn {
                error: Some(failure.text),
                ..SignIn::default()
            });
            // Ends the feed: nothing it fetches can succeed until there is a session again.
            self.feed = None;
            return Task::none();
        }

        match &mut self.screen {
            Screen::Connection(screen) => {
                screen.busy = false;
                screen.error = Some(failure.text);
            }
            Screen::SignIn(screen) => {
                screen.busy = false;
                screen.error = Some(failure.text);
            }
            Screen::Chat(screen) => screen.error = Some(failure.text),
        }
        Task::none()
    }
}

/// Builds the client, then proves the server is there.
///
/// Building it only parses configuration — nothing is dialled until a call is made — so the health
/// check is what turns a wrong address into an error on the connection screen rather than a
/// surprise on the next one.
async fn connect(url: String) -> Result<ChatClient, Failure> {
    let server_url = Url::parse(&url).map_err(|error| ChatError::Config(error.to_string()))?;
    let client = ChatClient::new(ClientConfig {
        transport: TransportKind::Tcp,
        server_url,
        ..ClientConfig::default()
    })
    .await?;
    client.health().await?;

    Ok(client)
}

/// Signs in, then reads the rooms the chat screen opens with.
async fn log_in(
    client: ChatClient,
    username: String,
    password: String,
) -> Result<Vec<Room>, Failure> {
    client.login(&username, &password).await?;

    Ok(client.rooms().await?)
}

/// One room's feed, as the stream a `Task` runs.
///
/// Opening and draining are one stream because they have to be: `watch_room` is fallible, and the
/// `RoomFeed` it returns can neither be cloned nor debugged, so it cannot travel in a `Message` to
/// be drained later.
fn watch(
    client: ChatClient,
    room: RoomId,
) -> impl Stream<Item = Result<Vec<ChatMessage>, Failure>> {
    stream::once(async move { client.watch_room(room, Since::Newest).await }).flat_map(|opened| {
        match opened {
            Ok(feed) => {
                feed.into_stream()
                    .map(|batch| batch.map_err(Failure::from))
                    .left_stream()
            }
            Err(error) => stream::once(async move { Err(Failure::from(error)) }).right_stream(),
        }
    })
}
