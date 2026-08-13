-- Copyright 2026 Anapaya Systems
--
-- Licensed under the Apache License, Version 2.0 (the "License");
-- you may not use this file except in compliance with the License.
-- You may obtain a copy of the License at
--
--   http://www.apache.org/licenses/LICENSE-2.0
--
-- Unless required by applicable law or agreed to in writing, software
-- distributed under the License is distributed on an "AS IS" BASIS,
-- WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
-- See the License for the specific language governing permissions and
-- limitations under the License.

-- Changing anything below means bumping SCHEMA_VERSION in store.rs. There are no migrations:
-- CREATE TABLE IF NOT EXISTS leaves an existing table untouched, so without the bump an old
-- database survives under new code. Bumping it deletes and rebuilds the database.

CREATE TABLE IF NOT EXISTS users (
    username   TEXT PRIMARY KEY COLLATE NOCASE,
    pw_hash    TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS rooms (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE COLLATE NOCASE,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    seq       INTEGER PRIMARY KEY,
    room_id   INTEGER NOT NULL REFERENCES rooms(id),
    username  TEXT NOT NULL,
    body      TEXT NOT NULL,
    posted_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_messages_room_seq ON messages(room_id, seq);
