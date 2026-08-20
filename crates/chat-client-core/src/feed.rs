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
//! A message poller for one room, behind a stream.
//!
//! The API has no push: new messages are found by asking for what came after the last `seq`. This
//! wraps that loop so a caller asks for the next batch instead of running a timer and a cursor.

use std::mem;

use chat_core::api::v1::{Message, RoomId, Seq};
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
pub struct RoomFeed {
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
    /// Opens a feed on a room, fetching the page it starts from.
    ///
    /// Fetching here is what turns a room that does not exist into an error, rather than a feed
    /// that never delivers.
    pub async fn watch_room(&self, room: RoomId, since: Since) -> Result<RoomFeed, ChatError> {
        let page = self.poll().page_size();
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
            holding,
            due: None,
        })
    }
}

impl RoomFeed {
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
                (!self.catching_up).then(|| Instant::now() + self.client.poll().room_interval)
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
