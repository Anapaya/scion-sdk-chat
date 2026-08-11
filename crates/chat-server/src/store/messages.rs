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
//! The messages table, and the three ways of paging it.

use chat_core::api::v1::{Message, PostMessageResponse, RoomId, Seq};

use super::{
    Store, StoreError,
    convert::{millis, now, seq, to_column},
};

impl Store {
    /// Appends a message to a room. The caller checks the room exists first; without that this
    /// fails on the foreign key.
    pub async fn post_message(
        &self,
        room: RoomId,
        username: &str,
        body: &str,
    ) -> Result<PostMessageResponse, StoreError> {
        let id = to_column("room id", room.get())?;
        let posted_at = to_column("timestamp", now().get())?;
        let row = sqlx::query!(
            r#"INSERT INTO messages (room_id, username, body, posted_at) VALUES (?, ?, ?, ?)
               RETURNING seq AS "seq!", posted_at AS "posted_at!""#,
            id,
            username,
            body,
            posted_at,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(PostMessageResponse {
            seq: seq(row.seq)?,
            posted_at: millis(row.posted_at)?,
        })
    }

    /// The newest `limit` messages in a room, oldest first — what opening a room fetches.
    pub async fn newest_page(&self, room: RoomId, limit: u32) -> Result<Vec<Message>, StoreError> {
        let id = to_column("room id", room.get())?;
        let limit = i64::from(limit);
        let rows = sqlx::query!(
            r#"SELECT seq AS "seq!", username, body, posted_at
               FROM messages WHERE room_id = ?
               ORDER BY seq DESC LIMIT ?"#,
            id,
            limit,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut messages = collect(
            rows.into_iter()
                .map(|row| (row.seq, row.username, row.body, row.posted_at)),
        )?;
        messages.reverse();
        Ok(messages)
    }

    /// Messages newer than `after`, oldest first — what polling fetches.
    pub async fn after(
        &self,
        room: RoomId,
        after: Seq,
        limit: u32,
    ) -> Result<Vec<Message>, StoreError> {
        let id = to_column("room id", room.get())?;
        let after = to_column("seq", after.get())?;
        let limit = i64::from(limit);
        let rows = sqlx::query!(
            r#"SELECT seq AS "seq!", username, body, posted_at
               FROM messages WHERE room_id = ? AND seq > ?
               ORDER BY seq ASC LIMIT ?"#,
            id,
            after,
            limit,
        )
        .fetch_all(&self.pool)
        .await?;

        collect(
            rows.into_iter()
                .map(|row| (row.seq, row.username, row.body, row.posted_at)),
        )
    }

    /// Messages older than `before`, oldest first — what "load more" fetches.
    pub async fn before(
        &self,
        room: RoomId,
        before: Seq,
        limit: u32,
    ) -> Result<Vec<Message>, StoreError> {
        let id = to_column("room id", room.get())?;
        let before = to_column("seq", before.get())?;
        let limit = i64::from(limit);
        let rows = sqlx::query!(
            r#"SELECT seq AS "seq!", username, body, posted_at
               FROM messages WHERE room_id = ? AND seq < ?
               ORDER BY seq DESC LIMIT ?"#,
            id,
            before,
            limit,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut messages = collect(
            rows.into_iter()
                .map(|row| (row.seq, row.username, row.body, row.posted_at)),
        )?;
        messages.reverse();
        Ok(messages)
    }
}

/// Turns the four columns every message query selects into [`Message`] values.
fn collect(
    rows: impl Iterator<Item = (i64, String, String, i64)>,
) -> Result<Vec<Message>, StoreError> {
    rows.map(|(s, username, body, posted_at)| {
        Ok(Message {
            seq: seq(s)?,
            username,
            body,
            posted_at: millis(posted_at)?,
        })
    })
    .collect()
}
