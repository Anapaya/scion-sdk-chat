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
//! Keeping one open room up to date.

use std::{mem, time::Duration};

use chat_core::api::v1::{Message, RoomId, Seq};
use futures::Stream;
use serde::Serialize;
use tokio::time::Instant;

use crate::{client::ChatClient, error::ChatError};

#[cfg(test)]
mod tests;

/// Longer than `room_interval`, or backing off would poll as often as not backing off.
const BACKOFF: Duration = Duration::from_secs(10);

/// Where watching a room starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Since {
    /// The newest `limit` messages.
    Newest {
        /// How many to fetch.
        limit: usize,
    },
    /// Everything after this position, exclusive.
    After(Seq),
}

/// Whether the feed is reaching the server. Reported on change, not per fetch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum ConnectionState {
    /// Fetching works.
    Healthy,
    /// The feed retries by itself; a caller shows this and does nothing.
    Degraded {
        /// What went wrong, for showing to a user.
        error: String,
        /// How long before the next attempt.
        retry_in: Duration,
    },
}

/// What a feed hands over.
///
/// `Serialize` so an interface that forwards events, a Tauri webview say, passes them on untouched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "event")]
pub enum RoomEvent {
    /// Oldest first, to append. The first batch is the backfill, and fires even when empty.
    Messages(Vec<Message>),
    /// Reaching the server has changed.
    Connection(ConnectionState),
    /// Terminal: the feed is over after this.
    SessionExpired,
}

/// One watched room, which fetches when asked for its next event.
///
/// There is no background task and no queue: stop calling [`next`](Self::next) and fetching stops.
/// Dropping the feed ends it. One consumer each — two views of a room means two feeds.
pub struct RoomFeed {
    client: ChatClient,
    room: RoomId,
    cursor: Seq,
    state: State,
    catching_up: bool,
    degraded: bool,
    /// A deadline rather than a duration, so a `next` dropped mid-wait resumes it. Otherwise a
    /// caller whose `select!` holds another timer of the same interval starves the feed: its
    /// deadlines are fixed, and a wait started afresh always lands later.
    due: Option<Instant>,
}

enum State {
    Holding(Vec<Message>),
    Fetching,
    Over,
}

impl ChatClient {
    /// Opens a feed on a room, fetching the page it starts from.
    ///
    /// Fetching here is what turns a room that does not exist into an error, rather than a feed
    /// that never delivers.
    pub async fn watch_room(&self, room: RoomId, since: Since) -> Result<RoomFeed, ChatError> {
        let page = self.poll().page_limit;
        let (backfill, from) = match since {
            Since::Newest { limit } => (self.messages_newest(room, limit).await?, Seq::START),
            Since::After(seq) => (self.messages_after(room, seq, page).await?, seq),
        };

        // Only a resume can have more waiting: the newest page is the end.
        let catching_up = matches!(since, Since::After(_)) && backfill.len() >= page;

        Ok(RoomFeed {
            client: self.clone(),
            room,
            cursor: backfill.last().map_or(from, |message| message.seq),
            state: State::Holding(backfill),
            catching_up,
            degraded: false,
            due: None,
        })
    }
}

impl RoomFeed {
    /// The room this feed watches.
    pub fn room(&self) -> RoomId {
        self.room
    }

    /// The next batch, or `None` once the feed is over.
    ///
    /// Cancel-safe: the cursor moves only after a batch has been decoded, and the wait is a
    /// deadline, so dropping this future loses neither messages nor elapsed time.
    pub async fn next(&mut self) -> Option<RoomEvent> {
        if matches!(self.state, State::Over) {
            return None;
        }
        if let State::Holding(messages) = mem::replace(&mut self.state, State::Fetching) {
            return Some(RoomEvent::Messages(messages));
        }

        let page = self.client.poll().page_limit;
        loop {
            if self.due.is_none() {
                let wait = match (self.degraded, self.catching_up) {
                    (true, _) => Some(BACKOFF),
                    // A full page means more is already waiting, so draining it does not pay the
                    // interval.
                    (false, true) => None,
                    (false, false) => Some(self.client.poll().room_interval),
                };
                self.due = wait.map(|wait| Instant::now() + wait);
            }
            if let Some(due) = self.due {
                tokio::time::sleep_until(due).await;
            }

            match self
                .client
                .messages_after(self.room, self.cursor, page)
                .await
            {
                Ok(messages) => {
                    let recovered = mem::take(&mut self.degraded);

                    self.due = None;
                    self.catching_up = messages.len() >= page;
                    if let Some(newest) = messages.last() {
                        self.cursor = newest.seq;
                    }

                    if recovered {
                        // One event per call, so the batch recovery came with waits for the next
                        // rather than being dropped.
                        self.state = State::Holding(messages);

                        return Some(RoomEvent::Connection(ConnectionState::Healthy));
                    }

                    return Some(RoomEvent::Messages(messages));
                }
                Err(ChatError::SessionExpired) => {
                    self.state = State::Over;
                    return Some(RoomEvent::SessionExpired);
                }
                // No classification: everything but a refused token is retried the same way, and
                // only the first is reported, so a server that is down produces one event.
                Err(error) => {
                    let first = !self.degraded;

                    self.degraded = true;
                    self.due = None;

                    if first {
                        return Some(RoomEvent::Connection(ConnectionState::Degraded {
                            error: error.to_string(),
                            retry_in: BACKOFF,
                        }));
                    }
                }
            }
        }
    }

    /// The same feed as a [`Stream`], for an interface whose subscription consumes one.
    ///
    /// Consuming rather than implementing `Stream`: `next` borrows the feed, so a `poll_next` would
    /// hold a future borrowing the struct it lives in.
    pub fn into_stream(self) -> impl Stream<Item = RoomEvent> {
        futures::stream::unfold(self, |mut feed| {
            async move { feed.next().await.map(|event| (event, feed)) }
        })
    }
}
