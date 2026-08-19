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
//! The typed API, and the session it carries.

use std::sync::{Arc, Mutex};

use bytes::Bytes;
use chat_core::api::v1::{
    CreateRoomRequest, LoginRequest, LoginResponse, Message, MessagesResponse, PostMessageRequest,
    PostMessageResponse, RegisterRequest, Room, RoomId, RoomsResponse, Seq, ServerInfo, UnixMillis,
};
use http::{
    Method, StatusCode,
    header::{AUTHORIZATION, CONTENT_TYPE},
};
use serde::{Serialize, de::DeserializeOwned};
use url::Url;

use crate::{
    config::{ClientConfig, PollConfig, TransportKind},
    error::{ChatError, refusal},
    transport::{Transport, tcp::TcpTransport},
};

#[cfg(test)]
mod tests;

/// A login, minus the token. The token never leaves the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    /// Who is logged in.
    pub username: String,
    /// When the login stops being accepted.
    pub expires_at: UnixMillis,
}

/// A login, token included.
struct Session {
    /// The bearer token every authenticated request carries.
    token: String,
    /// What a caller is allowed to see of it.
    info: SessionInfo,
}

/// Everything a client does, apart from watching a room.
///
/// Cheap to clone and safe to share across threads: every clone is the same client, session
/// included, so logging in on one is logging in on all of them.
#[derive(Clone)]
pub struct ChatClient {
    state: Arc<ClientState>,
}

struct ClientState {
    /// Which transport is used to reach the server.
    transport: Arc<dyn Transport>,
    /// The base every path is joined onto.
    server_url: Url,
    /// How often a room is polled, and how much is asked for.
    poll: PollConfig,
    /// The current login, if there is one. Never held across an `.await`.
    session: Mutex<Option<Session>>,
}

impl ChatClient {
    /// Builds a client that talks to a real server.
    ///
    /// Async because it runs inside the caller's runtime: it never creates one, never keeps a
    /// handle to one, and never spawns anything.
    pub async fn new(config: ClientConfig) -> Result<Self, ChatError> {
        let transport: Arc<dyn Transport> = match config.transport {
            TransportKind::Tcp => Arc::new(TcpTransport::new()?),
            TransportKind::Scion => {
                return Err(ChatError::Config(
                    "the scion transport is not implemented yet; use the tcp transport".to_owned(),
                ));
            }
        };

        Ok(Self::new_with_transport(
            transport,
            config.server_url,
            config.poll,
        ))
    }

    /// Builds a client around a transport the caller already has, which in practice is a mock.
    ///
    /// `server_url` is still needed because a request carries an absolute URL even when nothing
    /// dials it.
    pub fn new_with_transport(
        transport: Arc<dyn Transport>,
        server_url: Url,
        poll: PollConfig,
    ) -> Self {
        Self {
            state: Arc::new(ClientState {
                transport,
                server_url,
                poll,
                session: Mutex::new(None),
            }),
        }
    }

    /// How often a room is polled, and how much is asked for.
    pub fn poll(&self) -> &PollConfig {
        &self.state.poll
    }

    /// Creates an account. Does not log in: the two are separate, as the API is.
    pub async fn register(&self, username: &str, password: &str) -> Result<(), ChatError> {
        let body = RegisterRequest {
            username: username.to_owned(),
            password: password.to_owned(),
        };

        self.post::<_, IgnoredReply>("/api/v1/register", &body, Auth::None)
            .await
            .map(drop)
    }

    /// Exchanges a username and password for a session.
    pub async fn login(&self, username: &str, password: &str) -> Result<SessionInfo, ChatError> {
        let body = LoginRequest {
            username: username.to_owned(),
            password: password.to_owned(),
        };
        let reply: LoginResponse = self.post("/api/v1/login", &body, Auth::None).await?;

        Ok(self.store_session(username, reply))
    }

    /// The current login, or `None` when nobody is logged in.
    pub fn session(&self) -> Option<SessionInfo> {
        self.locked_session()
            .as_ref()
            .map(|session| session.info.clone())
    }

    /// Forgets the current login. The token is not revoked: nothing server-side holds it.
    pub fn logout(&self) {
        *self.locked_session() = None;
    }

    /// Checks that the server is running.
    pub async fn health(&self) -> Result<(), ChatError> {
        self.get::<IgnoredReply>("/api/v1/healthz", Auth::None)
            .await
            .map(drop)
    }

    /// Reads the version and the limits the server enforces. Needs no login, so a user interface
    /// can show them before anyone signs in.
    pub async fn server_info(&self) -> Result<ServerInfo, ChatError> {
        self.get("/api/v1/server", Auth::None).await
    }

    /// Lists every room.
    pub async fn rooms(&self) -> Result<Vec<Room>, ChatError> {
        Ok(self
            .get::<RoomsResponse>("/api/v1/rooms", Auth::Required)
            .await?
            .rooms)
    }

    /// Creates a room, or returns the one already holding the name.
    pub async fn create_room(&self, name: &str) -> Result<Room, ChatError> {
        let body = CreateRoomRequest {
            name: name.to_owned(),
        };

        self.post("/api/v1/rooms", &body, Auth::Required).await
    }

    /// Posts a message to a room.
    pub async fn send(&self, room: RoomId, body: &str) -> Result<PostMessageResponse, ChatError> {
        let body = PostMessageRequest {
            body: body.to_owned(),
        };

        self.post(&messages_path(room), &body, Auth::Required).await
    }

    /// Reads the newest `limit` messages in a room, oldest first.
    pub async fn messages_newest(
        &self,
        room: RoomId,
        limit: usize,
    ) -> Result<Vec<Message>, ChatError> {
        self.messages(room, limit, Cursor::Newest).await
    }

    /// Reads what arrived after `seq`, exclusive, oldest first.
    pub async fn messages_after(
        &self,
        room: RoomId,
        seq: Seq,
        limit: usize,
    ) -> Result<Vec<Message>, ChatError> {
        self.messages(room, limit, Cursor::After(seq)).await
    }

    /// Reads what came before `seq`, exclusive, oldest first. This is paging backwards through
    /// history, as a reader scrolling up asks for.
    pub async fn messages_before(
        &self,
        room: RoomId,
        seq: Seq,
        limit: usize,
    ) -> Result<Vec<Message>, ChatError> {
        self.messages(room, limit, Cursor::Before(seq)).await
    }

    /// One page of a room's messages, which the three named fetches differ only by.
    async fn messages(
        &self,
        room: RoomId,
        limit: usize,
        cursor: Cursor,
    ) -> Result<Vec<Message>, ChatError> {
        let page = format!("{}?limit={limit}", messages_path(room));
        let path = match cursor {
            Cursor::Newest => page,
            Cursor::After(seq) => format!("{page}&after_seq={seq}"),
            Cursor::Before(seq) => format!("{page}&before_seq={seq}"),
        };

        Ok(self
            .get::<MessagesResponse>(&path, Auth::Required)
            .await?
            .messages)
    }

    async fn get<Reply: DeserializeOwned>(
        &self,
        path: &str,
        auth: Auth,
    ) -> Result<Reply, ChatError> {
        let body = self.execute(Method::GET, path, None, auth).await?;

        decode(&body)
    }

    async fn post<Body: Serialize, Reply: DeserializeOwned>(
        &self,
        path: &str,
        body: &Body,
        auth: Auth,
    ) -> Result<Reply, ChatError> {
        let body = self
            .execute(Method::POST, path, Some(encode(body)?), auth)
            .await?;

        decode(&body)
    }

    /// Builds the request, sends it, and turns a refusal into an error.
    ///
    /// The only place a transport is touched, which is why the URL is joined, the token attached
    /// and the status read here and nowhere else.
    async fn execute(
        &self,
        method: Method,
        path: &str,
        body: Option<Bytes>,
        auth: Auth,
    ) -> Result<Bytes, ChatError> {
        let base = self.state.server_url.as_str().trim_end_matches('/');
        let mut request = http::Request::builder()
            .method(method)
            .uri(format!("{base}{path}"));

        if body.is_some() {
            request = request.header(CONTENT_TYPE, "application/json");
        }
        // Kept past the request, so the reply can tell whether the token it carried is still the
        // one stored.
        let mut sent = None;
        if auth == Auth::Required {
            let token = self.token().ok_or(ChatError::NotLoggedIn)?;
            request = request.header(AUTHORIZATION, format!("Bearer {token}"));
            sent = Some(token);
        }

        let request = request
            .body(body.unwrap_or_default())
            .map_err(|error| ChatError::Config(error.to_string()))?;

        let reply = self.state.transport.request(request).await?;
        let status = reply.status();
        let body = reply.into_body();

        if status.is_success() {
            Ok(body)
        } else if status == StatusCode::UNAUTHORIZED
            && let Some(token) = &sent
        {
            self.forget_if_current(token);
            Err(ChatError::SessionExpired)
        } else {
            Err(refusal(status.as_u16(), &body))
        }
    }

    /// Forgets the session, but only if it is still the one whose token was refused.
    fn forget_if_current(&self, refused: &str) {
        let mut session = self.locked_session();

        if session
            .as_ref()
            .is_some_and(|current| current.token == refused)
        {
            *session = None;
        }
    }

    fn token(&self) -> Option<String> {
        self.locked_session()
            .as_ref()
            .map(|session| session.token.clone())
    }

    /// Records a login and reports what a caller may see of it.
    fn store_session(&self, username: &str, reply: LoginResponse) -> SessionInfo {
        let info = SessionInfo {
            username: username.to_owned(),
            expires_at: reply.expires_at,
        };
        *self.locked_session() = Some(Session {
            token: reply.token,
            info: info.clone(),
        });

        info
    }

    fn locked_session(&self) -> std::sync::MutexGuard<'_, Option<Session>> {
        self.state
            .session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Where a room's messages are.
fn messages_path(room: RoomId) -> String {
    format!("/api/v1/rooms/{room}/messages")
}

/// Which messages a fetch asks for, beyond how many.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cursor {
    /// The newest page.
    Newest,
    /// What arrived after this position, exclusive.
    After(Seq),
    /// What came before this position, exclusive.
    Before(Seq),
}

/// Whether a request carries the bearer token.
///
/// An enum rather than a `bool` so a call site says which it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Auth {
    /// Attach the token, and refuse to send the request without one.
    Required,
    /// Send no token: liveness, server metadata, registering, logging in.
    None,
}

/// A reply whose contents are not used. Accepts any valid JSON and keeps none of it.
type IgnoredReply = serde::de::IgnoredAny;

fn encode<Body: Serialize>(body: &Body) -> Result<Bytes, ChatError> {
    serde_json::to_vec(body)
        .map(Bytes::from)
        .map_err(|error| ChatError::Protocol(error.to_string()))
}

/// Reads a reply.
///
/// A success that sends nothing — a 201 that says everything through its status — is read as the
/// empty JSON object, which is what a body with no fields in it is.
fn decode<Reply: DeserializeOwned>(body: &[u8]) -> Result<Reply, ChatError> {
    let body = if body.is_empty() { b"{}" } else { body };

    serde_json::from_slice(body).map_err(|error| ChatError::Protocol(error.to_string()))
}
