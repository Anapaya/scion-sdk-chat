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
use sqlx::{SqlitePool, error::ErrorKind};

use super::{
    super::StoreError,
    convert::{millis, now, seq, to_column},
};

/// Appends a message to a room.
pub(super) async fn post_message(
    pool: &SqlitePool,
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
    .fetch_one(pool)
    .await
    // The foreign key is what rejects a room that does not exist; report it as such rather
    // than passing a driver error up.
    .map_err(|e| {
        match &e {
            sqlx::Error::Database(db) if db.kind() == ErrorKind::ForeignKeyViolation => {
                StoreError::NotFound(format!("room {room}"))
            }
            _ => StoreError::DbError(e),
        }
    })?;

    Ok(PostMessageResponse {
        seq: seq(row.seq)?,
        posted_at: millis(row.posted_at)?,
    })
}

/// The newest `limit` messages in a room, oldest first.
pub(super) async fn newest(
    pool: &SqlitePool,
    room: RoomId,
    limit: u32,
) -> Result<Vec<Message>, StoreError> {
    let id = to_column("room id", room.get())?;
    let limit = i64::from(limit);
    let rows = sqlx::query!(
        r#"SELECT seq AS "seq!", username, body, posted_at
           FROM messages WHERE room_id = ?
           ORDER BY seq DESC LIMIT ?"#,
        id,
        limit,
    )
    .fetch_all(pool)
    .await?;

    let mut messages = collect(
        rows.into_iter()
            .map(|row| (row.seq, row.username, row.body, row.posted_at)),
    )?;
    messages.reverse();
    Ok(messages)
}

/// Messages newer than `after`, oldest first.
pub(super) async fn after(
    pool: &SqlitePool,
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
    .fetch_all(pool)
    .await?;

    collect(
        rows.into_iter()
            .map(|row| (row.seq, row.username, row.body, row.posted_at)),
    )
}

/// Messages older than `before`, oldest first.
pub(super) async fn before(
    pool: &SqlitePool,
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
    .fetch_all(pool)
    .await?;

    let mut messages = collect(
        rows.into_iter()
            .map(|row| (row.seq, row.username, row.body, row.posted_at)),
    )?;
    messages.reverse();
    Ok(messages)
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
