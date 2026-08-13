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
//! The accounts table.

use sqlx::SqlitePool;

use super::{
    super::StoreError,
    convert::{now, to_column},
};

/// Registers an account. Returns `false` when the username is already taken.
pub(super) async fn insert_user(
    pool: &SqlitePool,
    username: &str,
    pw_hash: &str,
) -> Result<bool, StoreError> {
    let created_at = to_column("timestamp", now().get())?;
    let result = sqlx::query!(
        "INSERT INTO users (username, pw_hash, created_at) VALUES (?, ?, ?)
         ON CONFLICT(username) DO NOTHING",
        username,
        pw_hash,
        created_at,
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected() == 1)
}
