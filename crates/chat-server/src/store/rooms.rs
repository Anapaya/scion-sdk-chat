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
//! The rooms table.

use chat_core::api::v1::{Room, RoomId};

use super::{
    Store, StoreError,
    convert::{now, room_id, seq, to_column},
};

/// The outcome of creating a room. Creation is idempotent on the name, so a taken name is a
/// success that reports the room already there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoomCreation {
    /// The room did not exist and was created.
    Created(Room),
    /// The name was taken; this is the room holding it.
    Existing(Room),
}

impl RoomCreation {
    /// The room, however it was obtained.
    pub fn room(&self) -> &Room {
        match self {
            Self::Created(room) | Self::Existing(room) => room,
        }
    }
}

impl Store {
    /// Creates a room, or returns the one already holding the name.
    pub async fn create_room(&self, name: &str) -> Result<RoomCreation, StoreError> {
        let created_at = to_column("timestamp", now().get())?;
        let inserted = sqlx::query!(
            "INSERT INTO rooms (name, created_at) VALUES (?, ?) ON CONFLICT(name) DO NOTHING",
            name,
            created_at,
        )
        .execute(&self.pool)
        .await?
        .rows_affected()
            == 1;

        let row = sqlx::query!(
            r#"SELECT r.id AS "id!", r.name AS "name!", COALESCE(MAX(m.seq), 0) AS "latest_seq!"
               FROM rooms r LEFT JOIN messages m ON m.room_id = r.id
               WHERE r.name = ?
               GROUP BY r.id, r.name"#,
            name,
        )
        .fetch_one(&self.pool)
        .await?;

        let room = Room {
            id: room_id(row.id)?,
            name: row.name,
            latest_seq: seq(row.latest_seq)?,
        };

        Ok(if inserted {
            RoomCreation::Created(room)
        } else {
            RoomCreation::Existing(room)
        })
    }

    /// Every room, oldest first, each with the `seq` of its newest message.
    pub async fn list_rooms(&self) -> Result<Vec<Room>, StoreError> {
        let rows = sqlx::query!(
            r#"SELECT r.id AS "id!", r.name AS "name!", COALESCE(MAX(m.seq), 0) AS "latest_seq!"
               FROM rooms r LEFT JOIN messages m ON m.room_id = r.id
               GROUP BY r.id, r.name
               ORDER BY r.id"#,
        )
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(Room {
                    id: room_id(row.id)?,
                    name: row.name,
                    latest_seq: seq(row.latest_seq)?,
                })
            })
            .collect()
    }

    /// Whether a room with this id exists.
    pub async fn room_exists(&self, room: RoomId) -> Result<bool, StoreError> {
        let id = to_column("room id", room.get())?;
        let row = sqlx::query!(
            r#"SELECT EXISTS(SELECT 1 FROM rooms WHERE id = ?) AS "found!""#,
            id,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(row.found != 0)
    }
}
