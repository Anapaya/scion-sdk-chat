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
//! Pollers behind a stream: one for a room's messages, one for the list of rooms.
//!
//! The API has no push, so staying current means asking again on a timer. These wrap that loop so
//! a caller asks for the next batch instead of owning a timer, a cursor and a retry.

use std::mem;

use chat_core::api::v1::{Message, Room, RoomId, Seq};
use futures::Stream;
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

/// One watched room, which fetches when asked for its next batch.
///
/// There is no background task and no queue: stop calling [`next`](Self::next) and fetching stops.
/// Dropping the feed ends it. One consumer each — two views of a room means two feeds.
pub struct MessagesFeed {
    client: ChatClient,
    room: RoomId,
    cursor: Seq,
    /// Fetched but not yet handed over.
    holding: Vec<Message>,
    catching_up: bool,
    /// A deadline rather than a duration, so a `next` dropped mid-wait resumes it. Otherwise a
    /// caller whose `select!` holds another timer of the same interval starves the feed: its
    /// deadlines are fixed, and a wait started afresh always lands later.
    due: Option<Instant>,
}

impl ChatClient {
    /// Opens a feed on one room's messages, fetching the page it starts from.
    ///
    /// Fetching here is what turns a room that does not exist into an error, rather than a feed
    /// that never delivers.
    pub async fn watch_room_messages(
        &self,
        room: RoomId,
        since: Since,
    ) -> Result<MessagesFeed, ChatError> {
        let page = self.poll().page_size();
        let (holding, from) = match since {
            Since::Newest => (self.messages_newest(room, page).await?, Seq::START),
            Since::After(seq) => (self.messages_after(room, seq, page).await?, seq),
        };

        Ok(MessagesFeed {
            client: self.clone(),
            room,
            cursor: holding.last().map_or(from, |message| message.seq),
            // Only a resume can have more waiting: the newest page is the end.
            catching_up: since != Since::Newest && holding.len() >= page,
            holding,
            due: None,
        })
    }
}

impl MessagesFeed {
    /// The room this feed watches.
    pub fn room(&self) -> RoomId {
        self.room
    }

    /// The next batch, oldest first, never empty.
    ///
    /// Waits until a message exists, so a caller sees nothing of the polling underneath.
    /// Cancel-safe: the cursor moves only after a batch has been decoded, and the wait is a
    /// deadline, so dropping this future loses neither messages nor elapsed time.
    pub async fn next(&mut self) -> Result<Vec<Message>, ChatError> {
        if !self.holding.is_empty() {
            return Ok(mem::take(&mut self.holding));
        }

        let page = self.client.poll().page_size();
        loop {
            self.due = self.due.or_else(|| {
                (!self.catching_up).then(|| Instant::now() + self.client.poll().messages_interval)
            });
            if let Some(due) = self.due {
                tokio::time::sleep_until(due).await;
            }

            let fetched = self
                .client
                .messages_after(self.room, self.cursor, page)
                .await;
            // Held until the fetch settles, so a drop anywhere above resumes this wait instead of
            // starting another.
            self.due = None;
            // Nothing arrived on a failure either, so there is nothing to chase and the next
            // attempt pays the interval.
            self.catching_up = matches!(&fetched, Ok(messages) if messages.len() >= page);

            let messages = fetched?;
            if let Some(newest) = messages.last() {
                self.cursor = newest.seq;

                return Ok(messages);
            }
        }
    }

    /// The same feed as a [`Stream`], for an interface whose subscription consumes one.
    ///
    /// Consuming rather than implementing `Stream`: `next` borrows the feed, so a `poll_next` would
    /// hold a future borrowing the struct it lives in.
    pub fn into_stream(self) -> impl Stream<Item = Result<Vec<Message>, ChatError>> {
        futures::stream::unfold(
            self,
            |mut feed| async move { Some((feed.next().await, feed)) },
        )
    }
}

/// The list of rooms, kept current the way [`MessagesFeed`] keeps a room's messages.
///
/// There is no background task and no queue: stop calling [`next`](Self::next) and fetching stops.
pub struct RoomsFeed {
    client: ChatClient,
    /// Fetched but not yet handed over.
    holding: Option<Vec<Room>>,
    /// A deadline rather than a duration, so a `next` dropped mid-wait resumes it. Otherwise a
    /// caller whose `select!` holds another timer of the same interval starves the feed: its
    /// deadlines are fixed, and a wait started afresh always lands later.
    due: Option<Instant>,
}

impl ChatClient {
    /// Opens a feed on the list of rooms, fetching the list it starts from.
    pub async fn watch_rooms(&self) -> Result<RoomsFeed, ChatError> {
        let rooms = self.rooms().await?;

        Ok(RoomsFeed {
            client: self.clone(),
            holding: Some(rooms),
            due: None,
        })
    }
}

impl RoomsFeed {
    /// The list as it now stands.
    ///
    /// Every read is handed over, unchanged or not: a list is the whole truth rather than a batch
    /// of new things, and a caller that hears nothing cannot tell a quiet server from a broken one.
    ///
    /// Cancel-safe: the wait is a deadline, so dropping this future loses no elapsed time, and a
    /// fetch dropped part-way is asked for again at once.
    pub async fn next(&mut self) -> Result<Vec<Room>, ChatError> {
        if let Some(rooms) = self.holding.take() {
            return Ok(rooms);
        }

        let due = *self
            .due
            .get_or_insert_with(|| Instant::now() + self.client.poll().rooms_interval);
        tokio::time::sleep_until(due).await;

        let fetched = self.client.rooms().await;
        // Held until the fetch settles, so a drop anywhere above resumes this wait instead of
        // starting another. A failure pays the interval again rather than being chased.
        self.due = None;

        fetched
    }

    /// The same feed as a [`Stream`], for an interface whose subscription consumes one.
    pub fn into_stream(self) -> impl Stream<Item = Result<Vec<Room>, ChatError>> {
        futures::stream::unfold(
            self,
            |mut feed| async move { Some((feed.next().await, feed)) },
        )
    }
}
