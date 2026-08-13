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
//! Store tests.

use std::path::PathBuf;

use chat_core::api::v1::{Message, Room, RoomId, Seq};
use tempfile::TempDir;

use super::{super::*, *};

/// A cap high enough that no test reaches it unless it means to.
const NO_CAP: u32 = u32::MAX;

/// The database file inside a test's directory.
fn db_path(dir: &TempDir) -> PathBuf {
    dir.path().join("chat.db")
}

/// A store on a fresh file, with the directory kept alive for the test's duration.
async fn store() -> (SqliteStore, TempDir) {
    let dir = TempDir::new().expect("temp dir");
    let store = SqliteStore::new(&db_path(&dir)).await.expect("open");
    (store, dir)
}

/// Closes the store and opens the same file again, as restarting the server does.
async fn restart(store: SqliteStore, dir: &TempDir) -> SqliteStore {
    drop(store);
    SqliteStore::new(&db_path(dir)).await.expect("reopen")
}

/// The room every client may assume exists.
async fn lobby(store: &SqliteStore) -> Room {
    store
        .list_rooms()
        .await
        .expect("list")
        .into_iter()
        .find(|room| room.name == LOBBY)
        .expect("lobby is seeded")
}

/// The trait exists so that callers can hold a store without naming the backend. That only
/// works if it stays usable behind a pointer.
#[tokio::test]
async fn the_store_is_usable_as_a_trait_object() {
    let (store, _dir) = store().await;
    let store: std::sync::Arc<dyn DataStore> = std::sync::Arc::new(store);

    assert_eq!(store.list_rooms().await.expect("list").len(), 1);
}

#[tokio::test]
async fn lobby_is_seeded_and_survives_a_restart() {
    let (store, dir) = store().await;
    let first = lobby(&store).await;
    store
        .post_message(first.id, "alice", "hi")
        .await
        .expect("post");

    let reopened = restart(store, &dir).await;
    let second = lobby(&reopened).await;

    assert_eq!(
        first.id, second.id,
        "lobby was recreated rather than reused"
    );
    assert_eq!(
        reopened
            .messages_newest(second.id, 10)
            .await
            .expect("page")
            .len(),
        1,
        "messages did not survive the restart"
    );
}

#[tokio::test]
async fn a_version_mismatch_rebuilds_the_database() {
    let (store, dir) = store().await;
    let room = store.create_room("scion", NO_CAP).await.expect("create");
    store
        .post_message(room.room().id, "alice", "hi")
        .await
        .expect("post");
    drop(store);

    // Stand in for a schema change: stamp a version this build does not recognise.
    let pool = connect(&db_path(&dir), false).await.expect("connect");
    sqlx::raw_sql("PRAGMA user_version = 999")
        .execute(&pool)
        .await
        .expect("stamp");
    pool.close().await;

    let store = SqliteStore::new(&db_path(&dir)).await.expect("reopen");
    let rooms = store.list_rooms().await.expect("list");

    assert_eq!(
        rooms.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
        [LOBBY],
        "the rebuilt database kept rooms from the old schema"
    );
    assert_eq!(store.list_rooms().await.expect("list").len(), 1);
}

#[tokio::test]
async fn opening_creates_the_directory_holding_the_database() {
    let dir = TempDir::new().expect("temp dir");
    let nested = dir.path().join("data").join("chat.db");

    let store = SqliteStore::new(&nested)
        .await
        .expect("open should create data/");

    assert!(nested.exists());
    assert_eq!(lobby(&store).await.name, LOBBY);
}

#[tokio::test]
async fn reopening_the_same_version_keeps_everything() {
    let (store, dir) = store().await;
    store.create_room("scion", NO_CAP).await.expect("create");

    let store = restart(store, &dir).await;
    assert_eq!(store.list_rooms().await.expect("list").len(), 2);
}

#[tokio::test]
async fn rooms_are_created_once_and_matched_case_insensitively() {
    let (store, _dir) = store().await;

    let created = store.create_room("scion", NO_CAP).await.expect("create");
    assert!(matches!(created, RoomCreation::Created(_)));

    let again = store
        .create_room("SCION", NO_CAP)
        .await
        .expect("create again");
    assert!(matches!(again, RoomCreation::Existing(_)));
    assert_eq!(created.room().id, again.room().id);
    assert_eq!(store.list_rooms().await.expect("list").len(), 2);
}

#[tokio::test]
async fn usernames_are_taken_once_and_case_insensitively() {
    let (store, _dir) = store().await;

    let hash = PasswordHash::new("hash".to_owned());
    assert_eq!(
        store
            .insert_user("alice", &hash, NO_CAP)
            .await
            .expect("insert"),
        Registration::Created
    );
    assert_eq!(
        store
            .insert_user("ALICE", &hash, NO_CAP)
            .await
            .expect("insert"),
        Registration::UsernameTaken,
        "the name should already be taken, case-insensitively"
    );
}

#[tokio::test]
async fn seq_increases_and_never_repeats_across_rooms() {
    let (store, _dir) = store().await;
    let lobby = lobby(&store).await;
    let other = store
        .create_room("scion", NO_CAP)
        .await
        .expect("create")
        .room()
        .id;

    let a = store
        .post_message(lobby.id, "alice", "1")
        .await
        .expect("post");
    let b = store.post_message(other, "bob", "2").await.expect("post");
    let c = store
        .post_message(lobby.id, "alice", "3")
        .await
        .expect("post");

    assert!(
        a.seq < b.seq && b.seq < c.seq,
        "seq must increase server-wide"
    );
    assert_eq!(
        store.messages_newest(lobby.id, 10).await.expect("page"),
        vec![
            Message {
                seq: a.seq,
                username: "alice".into(),
                body: "1".into(),
                posted_at: a.posted_at
            },
            Message {
                seq: c.seq,
                username: "alice".into(),
                body: "3".into(),
                posted_at: c.posted_at
            },
        ],
        "a room's page must skip the other room's seq"
    );
}

#[tokio::test]
async fn latest_seq_tracks_the_newest_message() {
    let (store, _dir) = store().await;
    let lobby = lobby(&store).await;

    assert_eq!(
        lobby.latest_seq,
        Seq::START,
        "an empty room starts at the sentinel"
    );

    let posted = store
        .post_message(lobby.id, "alice", "hi")
        .await
        .expect("post");
    let rooms = store.list_rooms().await.expect("list");
    let lobby = rooms.iter().find(|r| r.name == LOBBY).expect("lobby");

    assert_eq!(lobby.latest_seq, posted.seq);
}

#[tokio::test]
async fn the_three_fetch_shapes_agree_and_return_oldest_first() {
    let (store, _dir) = store().await;
    let room = lobby(&store).await.id;

    let mut seqs = Vec::new();
    for n in 0..5 {
        seqs.push(
            store
                .post_message(room, "alice", &n.to_string())
                .await
                .expect("post")
                .seq,
        );
    }

    let of = |page: Vec<Message>| page.into_iter().map(|m| m.seq).collect::<Vec<_>>();

    assert_eq!(
        of(store.messages_newest(room, 3).await.expect("newest")),
        seqs[2..]
    );
    assert_eq!(
        of(store.messages_newest(room, 50).await.expect("newest")),
        seqs
    );
    assert_eq!(
        of(store
            .messages_after(room, seqs[1], 50)
            .await
            .expect("after")),
        seqs[2..]
    );
    assert_eq!(
        of(store
            .messages_after(room, Seq::START, 50)
            .await
            .expect("after")),
        seqs
    );
    assert_eq!(
        of(store
            .messages_after(room, seqs[4], 50)
            .await
            .expect("after")),
        []
    );
    assert_eq!(
        of(store
            .messages_before(room, seqs[3], 2)
            .await
            .expect("before")),
        seqs[1..3]
    );
    assert_eq!(
        of(store
            .messages_before(room, seqs[0], 50)
            .await
            .expect("before")),
        []
    );
}

#[tokio::test]
async fn room_existence_is_reported_without_guessing() {
    let (store, _dir) = store().await;
    let lobby = lobby(&store).await;

    assert!(store.room_exists(lobby.id).await.expect("exists"));
    assert!(!store.room_exists(RoomId::new(9999)).await.expect("exists"));
}

#[tokio::test]
async fn posting_to_a_missing_room_is_refused() {
    let (store, _dir) = store().await;

    let result = store.post_message(RoomId::new(9999), "alice", "hi").await;

    assert!(
        matches!(result, Err(StoreError::NotFound(_))),
        "a missing room should be reported as not found, got {result:?}"
    );
}

#[tokio::test]
async fn a_cursor_beyond_the_column_range_is_an_error_not_a_panic() {
    let (store, _dir) = store().await;
    let room = lobby(&store).await.id;

    let result = store.messages_after(room, Seq::new(u64::MAX), 50).await;

    assert!(
        matches!(result, Err(StoreError::OutOfRange { what: "seq", .. })),
        "got {result:?}"
    );
}

/// The cap is enforced inside the insert, so it holds without a separate check the caller could
/// race against.
#[tokio::test]
async fn the_account_cap_refuses_the_one_past_it() {
    let (store, _dir) = store().await;
    let hash = PasswordHash::new("hash".to_owned());

    assert_eq!(
        store.insert_user("alice", &hash, 1).await.expect("insert"),
        Registration::Created
    );
    assert!(matches!(
        store.insert_user("bob", &hash, 1).await,
        Err(StoreError::CapExceeded { what: "account" })
    ));
}

#[tokio::test]
async fn the_room_cap_refuses_the_one_past_it() {
    let (store, _dir) = store().await;

    // The lobby already occupies the first slot.
    assert!(matches!(
        store.create_room("scion", 2).await.expect("create"),
        RoomCreation::Created(_)
    ));
    assert!(matches!(
        store.create_room("another", 2).await,
        Err(StoreError::CapExceeded { what: "room" })
    ));
    assert_eq!(store.list_rooms().await.expect("list").len(), 2);
}

/// A name that already exists is returned even at the cap: nothing is created, so nothing is
/// refused.
#[tokio::test]
async fn an_existing_room_is_returned_even_at_the_cap() {
    let (store, _dir) = store().await;

    let existing = store.create_room(LOBBY, 1).await.expect("create");

    assert!(matches!(existing, RoomCreation::Existing(_)));
    assert_eq!(existing.room().name, LOBBY);
}

/// The stored hash comes back for verification, and only for a name that exists.
#[tokio::test]
async fn the_password_hash_is_readable_for_verification() {
    let (store, _dir) = store().await;
    let hash = PasswordHash::new("$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA".to_owned());
    store
        .insert_user("alice", &hash, NO_CAP)
        .await
        .expect("insert");

    let stored = store.password_hash("Alice").await.expect("lookup");

    assert_eq!(stored.expect("a hash").as_str(), hash.as_str());
    assert!(store.password_hash("bob").await.expect("lookup").is_none());
}

/// A hash must not reach a log through the `Debug` of whatever carries it.
#[tokio::test]
async fn the_password_hash_is_redacted_in_debug_output() {
    let hash = PasswordHash::new("$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA".to_owned());

    let rendered = format!("{hash:?}");

    assert!(!rendered.contains("aGFzaA"), "{rendered}");
    assert!(rendered.contains("redacted"), "{rendered}");
}
