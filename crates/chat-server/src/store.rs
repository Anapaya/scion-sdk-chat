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
//! The store module defines the data access layer for the chat application.
//!
//! It provides the [DataStore] trait which abstracts the underlying storage mechanism, and
//! implementations for supported databases (currently SQLite).

use std::path::PathBuf;

use async_trait::async_trait;
use chat_core::api::v1::{Message, PostMessageResponse, Room, RoomId, Seq};
use thiserror::Error;

pub mod sqlite;

pub use self::sqlite::SqliteStore;

/// The room that always exists. Every implementation seeds it at startup, and no endpoint deletes
/// a room, so clients may assume it is there.
pub const LOBBY: &str = "lobby";

/// Anything the store can fail with.
#[derive(Debug, Error)]
pub enum StoreError {
    /// The row the caller named does not exist.
    #[error("not found: {0}")]
    NotFound(String),

    /// A query or a connection failed.
    #[error("database error: {0}")]
    DbError(#[from] sqlx::Error),

    /// The database file could not be created or removed.
    #[error("database file {path}: {source}")]
    FileError {
        /// The file being operated on.
        path: PathBuf,
        /// What the filesystem reported.
        source: std::io::Error,
    },

    /// A value did not fit across the boundary between the database's signed integers
    /// and the API's unsigned ones.
    #[error("{what} out of range: {value}")]
    OutOfRange {
        /// Which value.
        what: &'static str,
        /// What it held.
        value: i128,
    },
}

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

/// How much the server is holding, for the caps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    /// Registered accounts.
    pub users: u64,
    /// Rooms that exist.
    pub rooms: u64,
}

/// DataStore defines the interface for persisting chat data.
///
/// Implementations of this trait must be thread-safe ([`Send`] + [`Sync`]).
#[async_trait]
pub trait DataStore: Send + Sync {
    // ---- Accounts ----

    /// Register an account. Returns `false` when the username is already taken.
    async fn insert_user(&self, username: &str, pw_hash: &str) -> Result<bool, StoreError>;

    // ---- Rooms ----

    /// Create a room, or return the one already holding the name. Names are matched
    /// case-insensitively.
    async fn create_room(&self, name: &str) -> Result<RoomCreation, StoreError>;

    /// List every room, oldest first, each with the `seq` of its newest message.
    async fn list_rooms(&self) -> Result<Vec<Room>, StoreError>;

    /// Report whether a room with this id exists.
    async fn room_exists(&self, room: RoomId) -> Result<bool, StoreError>;

    // ---- Messages ----

    /// Append a message to a room.
    ///
    /// Returns [StoreError::NotFound] if the room does not exist.
    async fn post_message(
        &self,
        room: RoomId,
        username: &str,
        body: &str,
    ) -> Result<PostMessageResponse, StoreError>;

    /// Return the newest `limit` messages in a room, oldest first.
    async fn messages_newest(&self, room: RoomId, limit: u32) -> Result<Vec<Message>, StoreError>;

    /// Return the messages newer than `after`, oldest first.
    async fn messages_after(
        &self,
        room: RoomId,
        after: Seq,
        limit: u32,
    ) -> Result<Vec<Message>, StoreError>;

    /// Return the messages older than `before`, oldest first.
    async fn messages_before(
        &self,
        room: RoomId,
        before: Seq,
        limit: u32,
    ) -> Result<Vec<Message>, StoreError>;

    // ---- Caps ----

    /// Count the accounts and rooms held, for the caps.
    async fn counts(&self) -> Result<Counts, StoreError>;
}
