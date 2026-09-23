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

use std::{future::Future, io, path::PathBuf, time::Duration};

use chat_client_core::{
    ChatClient, ChatError, ClientConfig, MessagesFeed, PollConfig, RoomsFeed, ScionConfig, Since,
    SnapToken, TransportKind,
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
        connection::{Connection, ConnectionForm, Transport},
        sign_in::{self, SignIn},
    },
    ui::theme,
};

/// How often the open room is re-read. The sidebar keeps the default, being less urgent.
const MESSAGES_REFRESH: Duration = Duration::from_secs(1);

/// The most answers that may wait to be read. A bound, so a bug blocks instead of growing.
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

/// The calls that run away from the loop, so it keeps drawing while one is out.
struct Background {
    answers: mpsc::Receiver<Answer>,
    /// The end a call answers through. Cloned into every one of them.
    answer_to: mpsc::Sender<Answer>,
    /// Whether a call the user asked for is still out. One at a time, so sends keep their order.
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
    fn ask(&mut self, work: impl Future<Output = Answer> + Send + 'static) -> bool {
        if self.pending {
            return false;
        }
        self.pending = true;

        let answer_to = self.answer_to.clone();
        tokio::spawn(async move {
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
    /// The open room's messages.
    ///
    /// A stream, because `select!` drops the arms that did not win and a stream keeps a
    /// part-finished read inside itself. The request in flight survives a keypress.
    messages: Option<BoxStream<'static, Result<Vec<Message>, ChatError>>>,
    /// The sidebar's rooms, from the moment someone signs in.
    rooms: Option<BoxStream<'static, Result<Vec<Room>, ChatError>>>,
    /// Whether opening the room failed.
    reopen: bool,
    background: Background,
    exit: bool,
}

impl App {
    /// The app on its first screen, showing the form the command line filled in.
    pub fn new(form: ConnectionForm) -> Self {
        Self {
            screen: Screen::Connection(Connection::new(form)),
            client: None,
            messages: None,
            rooms: None,
            reopen: false,
            background: Background::default(),
            exit: false,
        }
    }

    /// Draws, then waits for a key, until asked to stop.
    pub async fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        let mut keys = EventStream::new();

        while !self.exit {
            terminal.draw(|frame| self.draw(frame))?;

            // Each arm produces what happened, so the select's borrows end before anything acts.
            let woken = tokio::select! {
                event = keys.next() => Woken::Terminal(event.transpose()?),
                batch = next_messages(&mut self.messages), if self.messages.is_some() => Woken::Messages(batch),
                list = next_rooms(&mut self.rooms), if self.rooms.is_some() => Woken::Rooms(list),
                Some(answer) = self.background.answers.recv() => Woken::Answer(answer),
            };

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
            // Answering at all is the sign the server is back.
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
        // Raw mode delivers Ctrl+C as a key, so ending the app on it is this loop's job.
        let quit = key.code == KeyCode::Esc
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(CONTROL));
        if quit {
            self.exit = true;
            return;
        }
        // The screens decide: only they know which of their keys reach a server.
        let pending = self.background.pending;

        match &mut self.screen {
            Screen::Connection(screen) => {
                let Some(form) = screen.handle_key(key, pending) else {
                    return;
                };
                self.connect(form);
            }
            Screen::SignIn(screen) => {
                let Some(intent) = screen.handle_key(key, pending) else {
                    return;
                };
                let (username, password) = screen.credentials();
                match intent {
                    sign_in::Intent::Register => self.register(&username, &password),
                    sign_in::Intent::LogIn => self.log_in(&username, &password),
                }
            }
            Screen::Chat(screen) => {
                let Some(intent) = screen.handle_key(key, pending) else {
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
        self.background.pending = false;

        match answer {
            Answer::Connected(Ok(client)) => {
                self.client = Some(client);
                self.screen = Screen::SignIn(SignIn::default());
            }
            // Read from the screen: a password is not something to put on a queue.
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
                self.refused(error);
            }
            // Nothing to do: the sidebar reads on its own and picks the room up.
            Answer::RoomCreated(Ok(())) => {}
            // Nothing to do either: the message arrives on the feed like everyone else's.
            Answer::MessageSent { result: Ok(()), .. } => {}
            // Asked for again by the next room list, which is the only clock this screen has.
            Answer::RoomOpened(Err(error)) => {
                self.reopen = true;
                self.refused(error);
            }
            Answer::Connected(Err(error))
            | Answer::Registered(Err(error))
            | Answer::LoggedIn {
                result: Err(error), ..
            }
            | Answer::RoomCreated(Err(error)) => self.refused(error),
        }
    }

    /// Builds the client, then proves the server is there.
    ///
    /// Nothing is dialled until a call is made, so the health check is what turns a wrong address
    /// into an error on this screen.
    fn connect(&mut self, form: ConnectionForm) {
        self.background.ask(async move {
            let built = async {
                let server_url = Url::parse(&form.server_url)
                    .map_err(|error| ChatError::Config(error.to_string()))?;
                let client = ChatClient::new(ClientConfig {
                    transport: transport(&server_url, &form)?,
                    server_url,
                    poll: PollConfig {
                        messages_interval: MESSAGES_REFRESH,
                        ..PollConfig::default()
                    },
                })
                .await?;
                client.health().await?;

                Ok::<_, ChatError>(client)
            };

            Answer::Connected(built.await)
        });
    }

    /// Creates the account. Signing in follows when the answer comes back.
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
                let listed = rooms.next().await?;

                Ok::<_, ChatError>((rooms, listed))
            }
            .await;

            Answer::LoggedIn { username, result }
        });
    }

    /// Watches the open room, dropping whatever was being watched before.
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

        // Asked for before the old room is let go, so a refusal leaves the pane filled.
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

    /// Posts a message. It arrives on the feed, which is what keeps every client in one order.
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
        if self.signed_out(&error) {
            return;
        }

        let message = error.to_string();
        match &mut self.screen {
            Screen::Connection(screen) => screen.error = Some(message),
            Screen::SignIn(screen) => screen.error = Some(message),
            Screen::Chat(screen) => screen.error = Some(message),
        }
    }

    /// Shows why a call the user asked for failed. Held until they ask for something else.
    fn refused(&mut self, error: ChatError) {
        if self.signed_out(&error) {
            return;
        }

        let message = error.to_string();
        match &mut self.screen {
            Screen::Chat(screen) => screen.warn(message),
            Screen::Connection(screen) => screen.error = Some(message),
            Screen::SignIn(screen) => screen.error = Some(message),
        }
    }

    /// Sends an ended session back to signing in, and says whether it did.
    fn signed_out(&mut self, error: &ChatError) -> bool {
        let gone = matches!(error, ChatError::SessionExpired | ChatError::NotLoggedIn);
        if !gone || !matches!(self.screen, Screen::Chat(_) | Screen::SignIn(_)) {
            return false;
        }

        let mut screen = SignIn::default();
        screen.error = Some(error.to_string());
        self.screen = Screen::SignIn(screen);
        self.messages = None;
        self.rooms = None;
        self.reopen = false;
        self.background = Background::default();

        true
    }
}

/// The open room's next batch.
///
/// A free function, because one `select!` waits on both and two methods would each borrow the
/// whole app. An ended stream is read as an ended session, the one thing that explains it.
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

/// The transport that was chosen, once the URL is checked against it.
fn transport(server_url: &Url, form: &ConnectionForm) -> Result<TransportKind, ChatError> {
    let wanted = form.transport.scheme();
    if server_url.scheme() != wanted {
        return Err(ChatError::Config(format!(
            "--transport {} is served over {wanted}, and this URL is {}. Change one of them.",
            form.transport.as_str(),
            server_url.scheme(),
        )));
    }

    match form.transport {
        Transport::Tcp => Ok(TransportKind::Tcp),
        Transport::Scion => {
            let endhost_api = blank_as_none(&form.endhost_api).ok_or_else(|| {
                ChatError::Config(
                    "SCION needs an endhost API: it is how the client finds the network. A local \
                     chat-dev prints one at startup."
                        .to_owned(),
                )
            })?;
            let endhost_api = Url::parse(&endhost_api).map_err(|error| {
                ChatError::Config(format!(
                    "the endhost API \"{endhost_api}\" is not a URL: {error}"
                ))
            })?;

            Ok(TransportKind::Scion(ScionConfig {
                endhost_api,
                snap_token: blank_as_none(form.snap_token.as_str()).map(SnapToken::new),
                target: blank_as_none(&form.target),
                cert_path: blank_as_none(&form.cert_path).map(PathBuf::from),
            }))
        }
    }
}

/// A field left blank, as the absence the client's configuration expects.
fn blank_as_none(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A form with every SCION field answered, as `chat-dev` answers them.
    fn answered(transport: Transport, server_url: &str) -> ConnectionForm {
        ConnectionForm {
            transport,
            server_url: server_url.to_owned(),
            endhost_api: "http://127.0.0.1:41234/".to_owned(),
            target: "2-ff00:0:212,127.0.0.1".to_owned(),
            cert_path: "/tmp/dev/cert.pem".to_owned(),
            snap_token: SnapToken::new("a token"),
        }
    }

    fn kind(form: &ConnectionForm) -> Result<TransportKind, ChatError> {
        transport(&Url::parse(&form.server_url).expect("a url"), form)
    }

    /// The flag decides. Each transport is served under one scheme, and the other is refused.
    #[test]
    fn the_flag_picks_the_transport_and_the_scheme_is_checked() {
        assert!(matches!(
            kind(&answered(Transport::Tcp, "http://localhost:8080")),
            Ok(TransportKind::Tcp)
        ));
        assert!(matches!(
            kind(&answered(Transport::Scion, "https://localhost:8443")),
            Ok(TransportKind::Scion(_))
        ));

        // Neither combination is one a user can mean: the server answers no TLS over TCP, and
        // SCION carries no HTTP/3 without it.
        assert!(matches!(
            kind(&answered(Transport::Tcp, "https://localhost:8080")),
            Err(ChatError::Config(_))
        ));
        assert!(matches!(
            kind(&answered(Transport::Scion, "http://localhost:8443")),
            Err(ChatError::Config(_))
        ));
    }

    /// The refusal names the flag, because the flag is the thing the reader chose.
    #[test]
    fn a_mismatch_says_which_flag_disagrees() {
        let Err(ChatError::Config(message)) =
            kind(&answered(Transport::Scion, "http://localhost:8443"))
        else {
            panic!("http over SCION cannot be served");
        };

        assert!(message.contains("--transport scion"), "{message}");
        assert!(message.contains("https"), "{message}");
    }

    /// A scheme neither transport uses is a mismatch like any other.
    #[test]
    fn an_unknown_scheme_is_refused() {
        assert!(matches!(
            kind(&answered(Transport::Tcp, "ftp://localhost")),
            Err(ChatError::Config(_))
        ));
    }

    #[test]
    fn the_scion_fields_are_carried_across() {
        let form = answered(Transport::Scion, "https://localhost:8443");

        let Ok(TransportKind::Scion(scion)) = kind(&form) else {
            panic!("scion with an endhost API is enough");
        };
        assert_eq!(scion.endhost_api.as_str(), form.endhost_api);
        assert_eq!(scion.target, Some(form.target));
        assert_eq!(scion.cert_path, Some(PathBuf::from(&form.cert_path)));
        assert_eq!(
            scion.snap_token.map(|token| token.as_str().to_owned()),
            Some(form.snap_token.as_str().to_owned())
        );
    }

    /// The three optional fields are optional. The endhost API is not, and says so.
    #[test]
    fn only_the_endhost_api_is_required_for_scion() {
        let bare = ConnectionForm {
            transport: Transport::Scion,
            server_url: "https://localhost:8443".to_owned(),
            endhost_api: "http://127.0.0.1:41234/".to_owned(),
            ..ConnectionForm::default()
        };

        let Ok(TransportKind::Scion(scion)) = kind(&bare) else {
            panic!("scion with an endhost API is enough");
        };
        assert_eq!(scion.target, None);
        assert_eq!(scion.cert_path, None);
        assert!(scion.snap_token.is_none());

        let blank = ConnectionForm {
            server_url: "https://localhost:8443".to_owned(),
            ..ConnectionForm::default()
        };
        assert!(matches!(kind(&blank), Err(ChatError::Config(_))));
    }

    /// The endhost API is refused under SCION alone: TCP has no use for one.
    #[test]
    fn tcp_needs_none_of_it() {
        let form = ConnectionForm {
            transport: Transport::Tcp,
            ..ConnectionForm::default()
        };

        assert!(matches!(kind(&form), Ok(TransportKind::Tcp)));
    }
}
