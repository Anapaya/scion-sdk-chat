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
//! A message poller for one room, behind an event stream.
//!
//! The API has no push: new messages are found by asking for what came after the last `seq`. This
//! wraps that loop so a caller asks for the next event instead of running a timer and a cursor.

use chat_core::api::v1::{Message, RoomId, Seq};
use futures::Stream;
use serde::Serialize;
use tokio::time::Instant;

use crate::{client::ChatClient, error::ChatError};

#[cfg(test)]
mod tests;

/// Where watching a room starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Since {
    /// A page of the newest messages.
    Newest,
    /// Everything after this position, exclusive.
    After(Seq),
}

/// What a feed hands over.
///
/// `Serialize` so an interface that forwards events, a Tauri webview say, passes them on untouched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomEvent {
    /// Oldest first, to append. The first batch is the backfill, and fires even when empty.
    Messages(Vec<Message>),
    /// A fetch failed. The feed tries again on its next call, and the batch after is the sign it
    /// recovered.
    Degraded(String),
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
    /// A page in hand, or `None` once it has been handed over.
    holding: Option<Vec<Message>>,
    catching_up: bool,
    over: bool,
    /// A deadline rather than a duration, so a `next` dropped mid-wait resumes it. Otherwise a
    /// caller whose `select!` holds another timer of the same interval starves the feed: its
    /// deadlines are fixed, and a wait started afresh always lands later.
    due: Option<Instant>,
}

impl ChatClient {
    /// Opens a feed on a room, fetching the page it starts from.
    ///
    /// Fetching here is what turns a room that does not exist into an error, rather than a feed
    /// that never delivers.
    pub async fn watch_room(&self, room: RoomId, since: Since) -> Result<RoomFeed, ChatError> {
        let page = self.poll().page_limit;
        let (holding, from) = match since {
            Since::Newest => (self.messages_newest(room, page).await?, Seq::START),
            Since::After(seq) => (self.messages_after(room, seq, page).await?, seq),
        };

        Ok(RoomFeed {
            client: self.clone(),
            room,
            cursor: holding.last().map_or(from, |message| message.seq),
            // Only a resume can have more waiting: the newest page is the end.
            catching_up: since != Since::Newest && holding.len() >= page,
            holding: Some(holding),
            over: false,
            due: None,
        })
    }
}

impl RoomFeed {
    /// The room this feed watches.
    pub fn room(&self) -> RoomId {
        self.room
    }

    /// The next event, or `None` once the feed is over.
    ///
    /// Cancel-safe: the cursor moves only after a batch has been decoded, and the wait is a
    /// deadline, so dropping this future loses neither messages nor elapsed time.
    pub async fn next(&mut self) -> Option<RoomEvent> {
        if self.over {
            return None;
        }
        if let Some(messages) = self.holding.take() {
            return Some(RoomEvent::Messages(messages));
        }

        let page = self.client.poll().page_limit;
        // A full page means more is already waiting, so draining it does not pay the interval.
        self.due = self.due.or_else(|| {
            (!self.catching_up).then(|| Instant::now() + self.client.poll().room_interval)
        });
        if let Some(due) = self.due {
            tokio::time::sleep_until(due).await;
        }
        self.due = None;

        match self
            .client
            .messages_after(self.room, self.cursor, page)
            .await
        {
            Ok(messages) => {
                self.catching_up = messages.len() >= page;
                if let Some(newest) = messages.last() {
                    self.cursor = newest.seq;
                }

                Some(RoomEvent::Messages(messages))
            }
            Err(ChatError::SessionExpired) => {
                self.over = true;

                Some(RoomEvent::SessionExpired)
            }
            // No classification: everything but a refused token is the same to a feed. Nothing
            // arrived, so there is nothing to chase and the next call pays the interval.
            Err(error) => {
                self.catching_up = false;

                Some(RoomEvent::Degraded(error.to_string()))
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
