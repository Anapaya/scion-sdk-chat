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

/// How long to wait after the first failed fetch.
const BACKOFF_START: Duration = Duration::from_secs(1);

/// The longest wait between attempts, whatever the failure.
const BACKOFF_MAX: Duration = Duration::from_secs(30);

/// Where watching a room starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Since {
    /// The newest `limit` messages: opening a room fresh.
    Newest {
        /// How many to fetch.
        limit: usize,
    },
    /// Everything after this position, exclusive: resuming where a client left off.
    After(Seq),
}

/// Whether the feed is reaching the server.
///
/// Reported when it changes, not on every fetch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum ConnectionState {
    /// Fetching is working again.
    Healthy,
    /// A fetch failed. The feed carries on by itself.
    Degraded {
        /// What went wrong, for showing to a user.
        error: String,
        /// How long the feed waits before trying again.
        retry_in: Duration,
    },
}

/// What a feed hands over.
///
/// `Serialize` so an interface that forwards events — a Tauri webview, say — can pass them on
/// untouched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "event")]
pub enum RoomEvent {
    /// A batch, oldest first, to append. The first one is the backfill, even when it is empty.
    Messages(Vec<Message>),
    /// The connection changed.
    Connection(ConnectionState),
    /// The token was refused. The feed is over after this.
    SessionExpired,
}

/// One watched room, which fetches when asked for its next event.
///
/// There is no background task and no queue: stop calling [`next`](Self::next) and fetching stops.
/// Dropping the feed ends it. One consumer each — two views of a room means two feeds.
pub struct RoomFeed {
    client: ChatClient,
    room: RoomId,
    /// The newest position handed over. The next fetch asks for what follows it.
    cursor: Seq,
    /// What the feed owes its caller next.
    state: State,
    /// Set when a page arrived full, so the next fetch skips the wait.
    catching_up: bool,
    /// How long to wait after a failure, and nothing while fetching works. Holding one is what it
    /// means to be degraded.
    backoff: Option<Duration>,
    /// When the next fetch is due.
    ///
    /// A deadline rather than a duration, so a `next` dropped part-way through its wait resumes
    /// that wait instead of starting it again. Without this a caller whose `select!` holds
    /// another timer of the same interval starves the feed: the timer's deadline is fixed
    /// while a fresh sleep is always later, so the fetch is never reached.
    due: Option<Instant>,
}

/// What the feed owes its caller next.
enum State {
    /// A page already fetched, waiting to be handed over.
    Holding(Vec<Message>),
    /// Nothing in hand: fetch on the next call.
    Fetching,
    /// Nothing more will come.
    Over,
}

impl ChatClient {
    /// Opens a feed on a room, fetching the page it starts from.
    ///
    /// The opening fetch is what turns a room that does not exist into an error here, rather than a
    /// feed that never delivers anything.
    pub async fn watch_room(&self, room: RoomId, since: Since) -> Result<RoomFeed, ChatError> {
        let page = self.poll().page_limit;
        let (backfill, from) = match since {
            Since::Newest { limit } => (self.messages_newest(room, limit).await?, Seq::START),
            Since::After(seq) => (self.messages_after(room, seq, page).await?, seq),
        };

        // Only a resume can have more waiting: the newest page is, by definition, the end.
        let catching_up = matches!(since, Since::After(_)) && backfill.len() >= page;

        Ok(RoomFeed {
            client: self.clone(),
            room,
            cursor: backfill.last().map_or(from, |message| message.seq),
            state: State::Holding(backfill),
            catching_up,
            backoff: None,
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
    /// Waits out the interval, fetches, and reports what happened. Cancel-safe: the cursor moves
    /// only after a batch has been decoded, so dropping this future loses nothing.
    pub async fn next(&mut self) -> Option<RoomEvent> {
        if matches!(self.state, State::Over) {
            return None;
        }
        // Whatever was held is handed over; a feed that held nothing is left as it was.
        if let State::Holding(messages) = mem::replace(&mut self.state, State::Fetching) {
            return Some(RoomEvent::Messages(messages));
        }

        let page = self.client.poll().page_limit;
        loop {
            if self.due.is_none() {
                self.due = self.wait().map(|wait| Instant::now() + wait);
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
                    self.due = None;

                    return Some(self.delivered(messages, page));
                }
                Err(ChatError::SessionExpired) => {
                    self.state = State::Over;
                    return Some(RoomEvent::SessionExpired);
                }
                // Every other failure is the same to a feed: back off and try again. Only the first
                // one is reported, so a server that is down does not fill a log with one event per
                // attempt.
                Err(error) => {
                    let first = self.backoff.is_none();
                    let retry_in = self.back_off();
                    self.due = None;

                    if first {
                        return Some(RoomEvent::Connection(ConnectionState::Degraded {
                            error: error.to_string(),
                            retry_in,
                        }));
                    }
                }
            }
        }
    }

    /// The same feed as a [`Stream`], for an interface whose subscription consumes one.
    ///
    /// Consuming rather than implementing `Stream` on the feed itself: `next` borrows the feed, so
    /// a `poll_next` would have to hold a future borrowing the struct it lives in.
    pub fn into_stream(self) -> impl Stream<Item = RoomEvent> {
        futures::stream::unfold(self, |mut feed| {
            async move { feed.next().await.map(|event| (event, feed)) }
        })
    }

    /// Records a batch and says what to report for it.
    fn delivered(&mut self, messages: Vec<Message>, page: usize) -> RoomEvent {
        let recovered = self.backoff.is_some();

        self.backoff = None;
        self.catching_up = messages.len() >= page;
        if let Some(newest) = messages.last() {
            self.cursor = newest.seq;
        }

        if recovered {
            // One call, one event, so recovery is reported alone — and the batch it came with waits
            // where the opening page waited, for the next call to take without a pause.
            self.state = State::Holding(messages);

            RoomEvent::Connection(ConnectionState::Healthy)
        } else {
            RoomEvent::Messages(messages)
        }
    }

    /// How long to wait before the next fetch.
    fn wait(&self) -> Option<Duration> {
        match (self.backoff, self.catching_up) {
            (Some(backoff), _) => Some(backoff),
            // A full page means more is already waiting, so draining it does not pay the interval.
            (None, true) => None,
            (None, false) => Some(self.client.poll().room_interval),
        }
    }

    /// Doubles the wait, to a ceiling.
    fn back_off(&mut self) -> Duration {
        let wait = self
            .backoff
            .map_or(BACKOFF_START, |last| (last * 2).min(BACKOFF_MAX));
        self.backoff = Some(wait);

        wait
    }
}
