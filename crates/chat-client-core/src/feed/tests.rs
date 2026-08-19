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
//! Tests for what a feed delivers, and when.
//!
//! Every one runs under a paused clock, so a minute of fetching takes microseconds and the gaps
//! between fetches can be asserted exactly rather than approximately.

use std::sync::Arc;

use chat_core::api::v1::ErrorCode;
use futures::StreamExt as _;
use url::Url;

use super::*;
use crate::{config::PollConfig, error::TransportError, transport::mock::MockTransport};

/// The route every fetch goes to.
const MESSAGES: &str = "GET /api/v1/rooms/1/messages";

/// The room every test watches.
fn room() -> RoomId {
    RoomId::new(1)
}

/// A page holding `seqs`, as the server would send it.
fn page(seqs: &[u64]) -> String {
    let messages: Vec<String> = seqs
        .iter()
        .map(|seq| {
            format!(
                r#"{{"seq":{seq},"username":"alice","body":"m{seq}","posted_at":1893456000000}}"#
            )
        })
        .collect();

    format!(r#"{{"messages":[{}]}}"#, messages.join(","))
}

/// A client over `mock`, logged in, fetching at most `page_limit` at a time.
async fn client(mock: &MockTransport, page_limit: usize) -> ChatClient {
    let client = ChatClient::new_with_transport(
        Arc::new(mock.clone()),
        Url::parse("http://host:8080").expect("a url"),
        PollConfig {
            room_interval: Duration::from_secs(2),
            page_limit,
        },
    );
    client.login("alice", "a password").await.expect("a login");

    client
}

/// A mock that answers a login, then the pages given, the last standing for every fetch after.
fn scripted(pages: &[String]) -> MockTransport {
    let mut mock = MockTransport::new().respond(
        "POST /api/v1/login",
        200,
        r#"{"token":"a.b.c","expires_at":1893456000000}"#,
    );

    for body in pages {
        mock = mock.respond(MESSAGES, 200, body.clone());
    }

    mock
}

/// The seqs a batch carried.
fn seqs(event: Option<RoomEvent>) -> Vec<u64> {
    let Some(RoomEvent::Messages(messages)) = event else {
        panic!("expected a batch, got {event:?}");
    };

    messages.iter().map(|message| message.seq.get()).collect()
}

#[tokio::test(start_paused = true)]
async fn the_first_event_is_the_page_watch_room_already_fetched() {
    let mock = scripted(&[page(&[1, 2])]);
    let client = client(&mock, 50).await;

    let mut feed = client
        .watch_room(room(), Since::Newest { limit: 50 })
        .await
        .expect("a feed");
    let fetched = mock.request_count();

    assert_eq!(seqs(feed.next().await), [1, 2]);
    assert_eq!(
        mock.request_count(),
        fetched,
        "the first call hands over what is already in hand",
    );
}

/// An empty opening page still fires: that is how a reader learns the backfill is done.
#[tokio::test(start_paused = true)]
async fn an_empty_backfill_is_still_delivered() {
    let mock = scripted(&[page(&[])]);
    let client = client(&mock, 50).await;
    let mut feed = client
        .watch_room(room(), Since::Newest { limit: 50 })
        .await
        .expect("a feed");

    assert_eq!(seqs(feed.next().await), [] as [u64; 0]);
}

/// A room that does not exist fails where the feed is opened, not silently ever after.
#[tokio::test(start_paused = true)]
async fn a_missing_room_fails_at_watch_room() {
    let refused = r#"{"error":{"code":"room_not_found","message":"no room with that id"}}"#;
    let mock = MockTransport::new()
        .respond(
            "POST /api/v1/login",
            200,
            r#"{"token":"a.b.c","expires_at":1893456000000}"#,
        )
        .respond(MESSAGES, 404, refused);
    let client = client(&mock, 50).await;

    // A feed holds a client, which holds a transport object, so it cannot be Debug and cannot go
    // through expect_err.
    let Err(error) = client.watch_room(room(), Since::Newest { limit: 50 }).await else {
        panic!("expected no such room");
    };

    assert!(
        matches!(
            error,
            ChatError::Api {
                code: ErrorCode::RoomNotFound,
                ..
            }
        ),
        "expected the room to be reported missing, got {error:?}",
    );
}

/// The acceptance criterion for cadence: one interval between fetches, and no more.
#[tokio::test(start_paused = true)]
async fn later_calls_wait_out_the_interval() {
    let mock = scripted(&[page(&[1]), page(&[])]);
    let client = client(&mock, 50).await;
    let mut feed = client
        .watch_room(room(), Since::Newest { limit: 50 })
        .await
        .expect("a feed");
    feed.next().await;

    for _ in 0..3 {
        feed.next().await;
    }

    let arrivals = mock.arrivals();
    let fetches: Vec<_> = arrivals[1..]
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect();
    assert_eq!(
        fetches,
        vec![Duration::from_secs(2); fetches.len()],
        "every gap is exactly the interval",
    );
}

/// A full page means more is already waiting, so draining it does not pay the interval.
#[tokio::test(start_paused = true)]
async fn a_full_page_is_followed_without_waiting() {
    let mock = scripted(&[page(&[1, 2]), page(&[3, 4]), page(&[5]), page(&[])]);
    let client = client(&mock, 2).await;
    let mut feed = client
        .watch_room(room(), Since::After(Seq::START))
        .await
        .expect("a feed");

    assert_eq!(
        seqs(feed.next().await),
        [1, 2],
        "the backfill filled the page"
    );
    assert_eq!(seqs(feed.next().await), [3, 4], "so does the next");
    assert_eq!(seqs(feed.next().await), [5], "and this one is short");
    assert_eq!(seqs(feed.next().await), [] as [u64; 0]);

    let arrivals = mock.arrivals();
    let gaps: Vec<_> = arrivals[1..]
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect();
    assert_eq!(
        gaps,
        [Duration::ZERO, Duration::ZERO, Duration::from_secs(2)],
        "both full pages are chased at once; only the short one pays the interval",
    );
}

/// The cursor follows what was delivered, so nothing arrives twice.
#[tokio::test(start_paused = true)]
async fn each_fetch_asks_from_the_newest_seq_delivered() {
    let mock = scripted(&[page(&[1, 2]), page(&[7])]);
    let client = client(&mock, 50).await;
    let mut feed = client
        .watch_room(room(), Since::Newest { limit: 50 })
        .await
        .expect("a feed");
    feed.next().await;

    feed.next().await;
    assert_eq!(
        mock.last_request().uri().query(),
        Some("limit=50&after_seq=2"),
    );

    feed.next().await;
    assert_eq!(
        mock.last_request().uri().query(),
        Some("limit=50&after_seq=7"),
    );
}

/// A failed fetch does not end the feed: it reports the trouble, waits, and carries on.
#[tokio::test(start_paused = true)]
async fn a_failure_is_reported_once_and_backs_off_to_a_ceiling() {
    let mock = MockTransport::new()
        .respond(
            "POST /api/v1/login",
            200,
            r#"{"token":"a.b.c","expires_at":1893456000000}"#,
        )
        .respond(MESSAGES, 200, page(&[]))
        .fail(MESSAGES, TransportError::Timeout);
    let client = client(&mock, 50).await;
    let mut feed = client
        .watch_room(room(), Since::Newest { limit: 50 })
        .await
        .expect("a feed");
    feed.next().await;

    let event = feed.next().await;

    let Some(RoomEvent::Connection(ConnectionState::Degraded { retry_in, error })) = event else {
        panic!("expected the connection to be reported, got {event:?}");
    };
    assert_eq!(retry_in, BACKOFF_START);
    assert!(
        error.contains("did not answer"),
        "it says what failed: {error}"
    );
}

/// Recovery is reported once, and the batch it arrived with is not lost.
#[tokio::test(start_paused = true)]
async fn recovery_is_reported_and_the_batch_survives_it() {
    let mock = MockTransport::new()
        .respond(
            "POST /api/v1/login",
            200,
            r#"{"token":"a.b.c","expires_at":1893456000000}"#,
        )
        .respond(MESSAGES, 200, page(&[]))
        .fail(MESSAGES, TransportError::Timeout)
        .respond(MESSAGES, 200, page(&[9]));
    let client = client(&mock, 50).await;
    let mut feed = client
        .watch_room(room(), Since::Newest { limit: 50 })
        .await
        .expect("a feed");
    feed.next().await;

    let degraded = feed.next().await;
    let healthy = feed.next().await;
    let batch = feed.next().await;

    assert!(matches!(
        degraded,
        Some(RoomEvent::Connection(ConnectionState::Degraded { .. }))
    ));
    assert_eq!(
        healthy,
        Some(RoomEvent::Connection(ConnectionState::Healthy)),
    );
    assert_eq!(seqs(batch), [9], "the batch recovery arrived with");
}

/// The acceptance criterion for the terminal event: a refused token ends the feed.
#[tokio::test(start_paused = true)]
async fn the_feed_ends_after_the_session_expires() {
    let expired = r#"{"error":{"code":"expired_token","message":"log in again"}}"#;
    let mock = MockTransport::new()
        .respond(
            "POST /api/v1/login",
            200,
            r#"{"token":"a.b.c","expires_at":1893456000000}"#,
        )
        .respond(MESSAGES, 200, page(&[]))
        .respond(MESSAGES, 401, expired);
    let client = client(&mock, 50).await;
    let mut feed = client
        .watch_room(room(), Since::Newest { limit: 50 })
        .await
        .expect("a feed");
    feed.next().await;

    assert_eq!(feed.next().await, Some(RoomEvent::SessionExpired));

    let sent = mock.request_count();
    assert_eq!(feed.next().await, None, "over for good");
    assert_eq!(mock.request_count(), sent, "and it stopped asking");
}

/// Stop calling and fetching stops: there is no task filling anything in the background.
#[tokio::test(start_paused = true)]
async fn nothing_is_fetched_until_it_is_asked_for() {
    let mock = scripted(&[page(&[1]), page(&[])]);
    let client = client(&mock, 50).await;
    let mut feed = client
        .watch_room(room(), Since::Newest { limit: 50 })
        .await
        .expect("a feed");
    feed.next().await;
    let fetched = mock.request_count();

    tokio::time::sleep(Duration::from_secs(60)).await;

    assert_eq!(mock.request_count(), fetched, "a minute, and nobody asked");
}

/// The same feed, read as a stream, for an interface whose subscription consumes one.
#[tokio::test(start_paused = true)]
async fn a_feed_reads_as_a_stream() {
    let expired = r#"{"error":{"code":"expired_token","message":"log in again"}}"#;
    let mock = MockTransport::new()
        .respond(
            "POST /api/v1/login",
            200,
            r#"{"token":"a.b.c","expires_at":1893456000000}"#,
        )
        .respond(MESSAGES, 200, page(&[1]))
        .respond(MESSAGES, 401, expired);
    let client = client(&mock, 50).await;
    let feed = client
        .watch_room(room(), Since::Newest { limit: 50 })
        .await
        .expect("a feed");

    let events: Vec<RoomEvent> = feed.into_stream().collect().await;

    assert_eq!(events.len(), 2, "the backfill, then the end: {events:?}");
    assert_eq!(events[1], RoomEvent::SessionExpired);
}

/// A feed sharing a `select!` with another timer still fetches.
///
/// This is what your caller's loop does. `select!` drops the losing future, so a wait started
/// afresh each call never completes against a timer whose deadline is fixed — the feed would report
/// the backfill and then nothing, for ever.
#[tokio::test(start_paused = true)]
async fn a_next_dropped_mid_wait_resumes_rather_than_restarts() {
    let mock = scripted(&[page(&[1]), page(&[2]), page(&[3])]);
    let client = client(&mock, 50).await;
    let mut feed = client
        .watch_room(room(), Since::Newest { limit: 50 })
        .await
        .expect("a feed");
    feed.next().await;

    // The same interval the app uses for its room list, racing the feed. Its handler takes a
    // moment, as a fetch does, which is what puts a freshly started wait behind the timer for
    // ever.
    let mut ticks = tokio::time::interval(Duration::from_secs(2));
    let mut batches = 0;
    for _ in 0..6 {
        tokio::select! {
            event = feed.next() => {
                if matches!(event, Some(RoomEvent::Messages(_))) {
                    batches += 1;
                }
            }
            _ = ticks.tick() => tokio::time::sleep(Duration::from_millis(1)).await,
        }
    }

    assert!(batches > 0, "the feed never reached a fetch");
}
