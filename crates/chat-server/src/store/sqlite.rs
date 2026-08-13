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
//! The SQLite implementation of [`DataStore`].
//!
//! This file opens the database and owns the pool. The statements live one module down, grouped
//! by table, and the trait implementation below is only dispatch.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use async_trait::async_trait;
use chat_core::api::v1::{Message, PostMessageResponse, Room, RoomId, Seq};
use sqlx::{
    AssertSqlSafe, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

use super::{DataStore, LOBBY, PasswordHash, Registration, RoomCreation, StoreError};

mod convert;
mod messages;
mod rooms;
#[cfg(test)]
mod tests;
mod users;

/// The schema applied at startup. There are no migrations.
const SCHEMA: &str = include_str!("sqlite/schema.sql");

/// Stamped into `PRAGMA user_version`. A database stamped with anything else is deleted and
/// rebuilt, so bumping this discards all data. Bump it whenever `schema.sql` changes.
const SCHEMA_VERSION: i32 = 1;

/// A [`DataStore`] backed by a SQLite file.
#[derive(Debug, Clone)]
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    /// Opens the database at `path`, creating it and its directory when absent and **discarding
    /// it** when it was written by a different schema version. Applies the schema and seeds
    /// [`LOBBY`].
    pub async fn new(path: &Path) -> Result<Self, StoreError> {
        // SQLite creates the file but not the directory holding it, and reports the difference
        // only as "unable to open database file".
        match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => {
                std::fs::create_dir_all(parent).map_err(|source| {
                    StoreError::FileError {
                        path: parent.to_path_buf(),
                        source,
                    }
                })?;
            }
            _ => {}
        }

        if stamped_version(path)
            .await?
            .is_some_and(|v| v != SCHEMA_VERSION)
        {
            remove_database(path)?;
        }

        let pool = connect(path, true).await?;
        sqlx::raw_sql(SCHEMA).execute(&pool).await?;
        // A PRAGMA takes no bind parameter, so the statement is built by hand. Safe to assert:
        // SCHEMA_VERSION is a compile-time integer constant, not caller input.
        sqlx::query(AssertSqlSafe(format!(
            "PRAGMA user_version = {SCHEMA_VERSION}"
        )))
        .execute(&pool)
        .await?;

        let store = Self { pool };
        // The lobby is seeded before any cap can apply to it.
        store.create_room(LOBBY, u32::MAX).await?;
        Ok(store)
    }
}

#[async_trait]
impl DataStore for SqliteStore {
    async fn insert_user(
        &self,
        username: &str,
        pw_hash: &PasswordHash,
        max_accounts: u32,
    ) -> Result<Registration, StoreError> {
        users::insert_user(&self.pool, username, pw_hash, max_accounts).await
    }

    async fn password_hash(&self, username: &str) -> Result<Option<PasswordHash>, StoreError> {
        users::password_hash(&self.pool, username).await
    }

    async fn create_room(&self, name: &str, max_rooms: u32) -> Result<RoomCreation, StoreError> {
        rooms::create_room(&self.pool, name, max_rooms).await
    }

    async fn list_rooms(&self) -> Result<Vec<Room>, StoreError> {
        rooms::list_rooms(&self.pool).await
    }

    async fn room_exists(&self, room: RoomId) -> Result<bool, StoreError> {
        rooms::room_exists(&self.pool, room).await
    }

    async fn post_message(
        &self,
        room: RoomId,
        username: &str,
        body: &str,
    ) -> Result<PostMessageResponse, StoreError> {
        messages::post_message(&self.pool, room, username, body).await
    }

    async fn messages_newest(&self, room: RoomId, limit: u32) -> Result<Vec<Message>, StoreError> {
        messages::newest(&self.pool, room, limit).await
    }

    async fn messages_after(
        &self,
        room: RoomId,
        after: Seq,
        limit: u32,
    ) -> Result<Vec<Message>, StoreError> {
        messages::after(&self.pool, room, after, limit).await
    }

    async fn messages_before(
        &self,
        room: RoomId,
        before: Seq,
        limit: u32,
    ) -> Result<Vec<Message>, StoreError> {
        messages::before(&self.pool, room, before, limit).await
    }
}

/// Opens a pool with the settings every connection needs.
async fn connect(path: &Path, create: bool) -> Result<SqlitePool, StoreError> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(create)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true);

    Ok(SqlitePoolOptions::new().connect_with(options).await?)
}

/// The `user_version` an existing database is stamped with. `None` when there is no database, or
/// when it reads as `0` — the value a file gets before any schema is stamped into it.
async fn stamped_version(path: &Path) -> Result<Option<i32>, StoreError> {
    if !path.exists() {
        return Ok(None);
    }

    let pool = connect(path, false).await?;
    let version: i32 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&pool)
        .await?;
    pool.close().await;

    Ok((version != 0).then_some(version))
}

/// Deletes the database and the files WAL mode keeps beside it.
fn remove_database(path: &Path) -> Result<(), StoreError> {
    for suffix in ["", "-wal", "-shm"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        let file = PathBuf::from(name);
        match std::fs::remove_file(&file) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(StoreError::FileError { path: file, source }),
        }
    }
    Ok(())
}
