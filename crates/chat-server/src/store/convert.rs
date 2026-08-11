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
//! Crossing between SQLite's signed integers and the API's unsigned ones, and reading the clock.

use std::time::{SystemTime, UNIX_EPOCH};

use chat_core::api::v1::{RoomId, Seq, UnixMillis};

use super::StoreError;

/// Now, on the server's clock.
pub(super) fn now() -> UnixMillis {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the system clock is set before the Unix epoch")
        .as_millis();
    UnixMillis::new(u64::try_from(millis).expect("the clock is past u64 milliseconds"))
}

/// A column on its way out. SQLite integers are signed and the API's are not, so this is where a
/// negative column value is caught.
pub(super) fn from_column(what: &'static str, value: i64) -> Result<u64, StoreError> {
    u64::try_from(value).map_err(|_| {
        StoreError::OutOfRange {
            what,
            value: i128::from(value),
        }
    })
}

/// A value on its way into a column. Fails only above `i64::MAX`, which a client can ask for by
/// sending an enormous cursor.
pub(super) fn to_column(what: &'static str, value: u64) -> Result<i64, StoreError> {
    i64::try_from(value).map_err(|_| {
        StoreError::OutOfRange {
            what,
            value: i128::from(value),
        }
    })
}

pub(super) fn room_id(value: i64) -> Result<RoomId, StoreError> {
    from_column("room id", value).map(RoomId::new)
}

pub(super) fn seq(value: i64) -> Result<Seq, StoreError> {
    from_column("seq", value).map(Seq::new)
}

pub(super) fn millis(value: i64) -> Result<UnixMillis, StoreError> {
    from_column("timestamp", value).map(UnixMillis::new)
}
