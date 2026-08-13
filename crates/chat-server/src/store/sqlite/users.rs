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
    super::{PasswordHash, Registration, StoreError},
    convert::{now, to_column},
};

/// Registers an account, refusing once `max_accounts` exist.
///
/// The cap is checked inside the insert rather than before it, so two concurrent registrations
/// cannot both pass a check and both write.
pub(super) async fn insert_user(
    pool: &SqlitePool,
    username: &str,
    pw_hash: &PasswordHash,
    max_accounts: u32,
) -> Result<Registration, StoreError> {
    let created_at = to_column("timestamp", now().get())?;
    let hash = pw_hash.as_str();
    let max = i64::from(max_accounts);

    let inserted = sqlx::query!(
        "INSERT INTO users (username, pw_hash, created_at)
         SELECT ?, ?, ? WHERE (SELECT COUNT(*) FROM users) < ?
         ON CONFLICT(username) DO NOTHING",
        username,
        hash,
        created_at,
        max,
    )
    .execute(pool)
    .await?
    .rows_affected()
        == 1;

    if inserted {
        return Ok(Registration::Created);
    }

    // Nothing was written, for one of two reasons: the name is taken, or the cap is reached.
    let taken = sqlx::query!(
        r#"SELECT EXISTS(SELECT 1 FROM users WHERE username = ?) AS "found!""#,
        username,
    )
    .fetch_one(pool)
    .await?
    .found
        != 0;

    if taken {
        Ok(Registration::UsernameTaken)
    } else {
        Err(StoreError::CapExceeded { what: "account" })
    }
}

/// The stored hash for an account, or `None` when no such account exists.
pub(super) async fn password_hash(
    pool: &SqlitePool,
    username: &str,
) -> Result<Option<PasswordHash>, StoreError> {
    let row = sqlx::query!("SELECT pw_hash FROM users WHERE username = ?", username)
        .fetch_optional(pool)
        .await?;

    Ok(row.map(|row| PasswordHash::new(row.pw_hash)))
}
