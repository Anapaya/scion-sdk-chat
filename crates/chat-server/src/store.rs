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
//! SQLite persistence. This file opens the database and owns the pool; the statements live one
//! module down, grouped by table — `users`, `rooms`, `messages` — and nowhere else.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use sqlx::{
    AssertSqlSafe, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

mod convert;
mod messages;
mod rooms;
#[cfg(test)]
mod tests;
mod users;

pub use rooms::RoomCreation;

/// The schema applied at startup. There are no migrations.
const SCHEMA: &str = include_str!("schema.sql");

/// Stamped into `PRAGMA user_version`. A database stamped with anything else is deleted and
/// rebuilt, so bumping this discards all data. Bump it whenever `schema.sql` changes.
const SCHEMA_VERSION: i32 = 1;

/// The room that always exists. Seeded at every startup, and no endpoint deletes a room.
pub const LOBBY: &str = "lobby";

/// Anything the store can fail with.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// A query or a connection failed.
    #[error("database: {0}")]
    Database(#[from] sqlx::Error),
    /// The database file could not be removed.
    #[error("database file {path}: {source}")]
    File {
        /// The file being removed.
        path: PathBuf,
        /// What the filesystem reported.
        source: std::io::Error,
    },
    /// A value did not fit across the SQLite boundary, where signed integers meet the API's
    /// unsigned ones.
    #[error("{what} out of range: {value}")]
    OutOfRange {
        /// Which value.
        what: &'static str,
        /// What it held.
        value: i128,
    },
}

/// How much the server is holding, for the caps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    /// Registered accounts.
    pub users: u64,
    /// Rooms that exist.
    pub rooms: u64,
}

/// The database, and every query against it.
#[derive(Debug, Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    /// Opens the database at `path`, creating it and its directory when absent and
    /// **discarding it** when it was written by a different schema version. Applies the schema
    /// and seeds [`LOBBY`].
    pub async fn new(path: &Path) -> Result<Self, StoreError> {
        // SQLite creates the file but not the directory holding it, and reports the difference
        // only as "unable to open database file".
        match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => {
                std::fs::create_dir_all(parent).map_err(|source| {
                    StoreError::File {
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
        // A PRAGMA takes no bind parameter, so the statement is built by hand. Safe to
        // assert: SCHEMA_VERSION is a compile-time integer constant, not caller input.
        sqlx::query(AssertSqlSafe(format!(
            "PRAGMA user_version = {SCHEMA_VERSION}"
        )))
        .execute(&pool)
        .await?;

        let store = Self { pool };
        store.create_room(LOBBY).await?;
        Ok(store)
    }

    /// Accounts and rooms held, for the caps. Spans both tables, so it lives here rather than in
    /// either of their modules.
    pub async fn counts(&self) -> Result<Counts, StoreError> {
        let row = sqlx::query!(
            r#"SELECT (SELECT COUNT(*) FROM users) AS "users!",
                      (SELECT COUNT(*) FROM rooms) AS "rooms!""#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(Counts {
            users: convert::from_column("user count", row.users)?,
            rooms: convert::from_column("room count", row.rooms)?,
        })
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
            Err(source) => return Err(StoreError::File { path: file, source }),
        }
    }
    Ok(())
}
