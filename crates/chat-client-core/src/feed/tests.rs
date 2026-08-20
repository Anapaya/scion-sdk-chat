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

use std::{sync::Arc, time::Duration};

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
fn seqs(batch: Result<Vec<Message>, ChatError>) -> Vec<u64> {
    batch
        .expect("a batch")
        .iter()
        .map(|message| message.seq.get())
        .collect()
}

#[tokio::test(start_paused = true)]
async fn the_first_batch_is_the_page_watch_room_already_fetched() {
    let mock = scripted(&[page(&[1, 2])]);
    let client = client(&mock, 50).await;

    let mut feed = client
        .watch_room(room(), Since::Newest)
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

    let Err(error) = client.watch_room(room(), Since::Newest).await else {
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

/// A batch is never empty: the poll underneath is not something a caller should see.
#[tokio::test(start_paused = true)]
async fn an_empty_page_is_waited_through_rather_than_handed_over() {
    let mock = scripted(&[page(&[]), page(&[]), page(&[7])]);
    let client = client(&mock, 50).await;
    let mut feed = client
        .watch_room(room(), Since::Newest)
        .await
        .expect("a feed");

    assert_eq!(
        seqs(feed.next().await),
        [7],
        "the empty pages never surfaced"
    );
    assert_eq!(mock.request_count(), 4, "one open, three fetches");
}

/// One interval between fetches, and no more.
#[tokio::test(start_paused = true)]
async fn fetches_wait_out_the_interval() {
    let mock = scripted(&[page(&[1]), page(&[]), page(&[2])]);
    let client = client(&mock, 50).await;
    let mut feed = client
        .watch_room(room(), Since::Newest)
        .await
        .expect("a feed");
    let _ = feed.next().await;

    let _ = feed.next().await;

    let arrivals = mock.arrivals();
    let gaps: Vec<_> = arrivals[1..]
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect();
    assert_eq!(
        gaps,
        vec![Duration::from_secs(2); gaps.len()],
        "every gap is exactly the interval",
    );
}

/// A full page means more is already waiting, so draining it does not pay the interval.
#[tokio::test(start_paused = true)]
async fn a_full_page_is_followed_without_waiting() {
    let mock = scripted(&[page(&[1, 2]), page(&[3, 4]), page(&[5])]);
    let client = client(&mock, 2).await;
    let mut feed = client
        .watch_room(room(), Since::After(Seq::START))
        .await
        .expect("a feed");

    assert_eq!(seqs(feed.next().await), [1, 2]);
    assert_eq!(seqs(feed.next().await), [3, 4]);
    assert_eq!(seqs(feed.next().await), [5]);

    let arrivals = mock.arrivals();
    let gaps: Vec<_> = arrivals[1..]
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect();
    assert_eq!(gaps, [Duration::ZERO, Duration::ZERO]);
}

/// The cursor follows what was delivered, so nothing arrives twice.
#[tokio::test(start_paused = true)]
async fn each_fetch_asks_from_the_newest_seq_delivered() {
    let mock = scripted(&[page(&[1, 2]), page(&[7])]);
    let client = client(&mock, 50).await;
    let mut feed = client
        .watch_room(room(), Since::Newest)
        .await
        .expect("a feed");
    let _ = feed.next().await;

    let _ = feed.next().await;
    assert_eq!(
        mock.last_request().uri().query(),
        Some("limit=50&after_seq=2"),
    );

    let _ = feed.next().await;
    assert_eq!(
        mock.last_request().uri().query(),
        Some("limit=50&after_seq=7"),
    );
}

/// A failure reaches the caller as an error, and the feed is still usable after it.
#[tokio::test(start_paused = true)]
async fn a_failure_is_returned_and_the_feed_carries_on() {
    let mock = MockTransport::new()
        .respond(
            "POST /api/v1/login",
            200,
            r#"{"token":"a.b.c","expires_at":1893456000000}"#,
        )
        .respond(MESSAGES, 200, page(&[1]))
        .fail(MESSAGES, TransportError::Timeout)
        .respond(MESSAGES, 200, page(&[9]));
    let client = client(&mock, 50).await;
    let mut feed = client
        .watch_room(room(), Since::Newest)
        .await
        .expect("a feed");
    let _ = feed.next().await;

    let failed = feed.next().await;
    assert!(
        matches!(failed, Err(ChatError::Transport(TransportError::Timeout))),
        "expected the timeout to reach the caller, got {failed:?}",
    );
    assert_eq!(seqs(feed.next().await), [9], "and the next call works");
}

/// A failure while catching up must still pay the interval, or the feed spins on a server that is
/// already struggling.
#[tokio::test(start_paused = true)]
async fn failing_mid_catch_up_still_waits() {
    let mock = MockTransport::new()
        .respond(
            "POST /api/v1/login",
            200,
            r#"{"token":"a.b.c","expires_at":1893456000000}"#,
        )
        .respond(MESSAGES, 200, page(&[1, 2]))
        .fail(MESSAGES, TransportError::Timeout);
    let client = client(&mock, 2).await;
    let mut feed = client
        .watch_room(room(), Since::After(Seq::START))
        .await
        .expect("a feed");
    let _ = feed.next().await;

    for _ in 0..3 {
        let _ = feed.next().await;
    }

    let arrivals = mock.arrivals();
    let gaps: Vec<_> = arrivals[1..]
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect();
    assert!(
        gaps.iter().skip(1).all(|gap| *gap > Duration::ZERO),
        "a failure must not be chased at once: {gaps:?}",
    );
}

/// A refused token is an error like any other: the caller decides it is terminal.
#[tokio::test(start_paused = true)]
async fn a_refused_token_is_returned_as_an_error() {
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
    let mut feed = client
        .watch_room(room(), Since::Newest)
        .await
        .expect("a feed");
    let _ = feed.next().await;

    let error = feed.next().await;

    assert!(
        matches!(error, Err(ChatError::SessionExpired)),
        "expected the session to end, got {error:?}",
    );
}

/// Stop calling and fetching stops: there is no task filling anything in the background.
#[tokio::test(start_paused = true)]
async fn nothing_is_fetched_until_it_is_asked_for() {
    let mock = scripted(&[page(&[1])]);
    let client = client(&mock, 50).await;
    let mut feed = client
        .watch_room(room(), Since::Newest)
        .await
        .expect("a feed");
    let _ = feed.next().await;
    let fetched = mock.request_count();

    tokio::time::sleep(Duration::from_secs(60)).await;

    assert_eq!(mock.request_count(), fetched, "a minute, and nobody asked");
}

/// A `next` dropped mid-wait resumes it, or a caller whose `select!` holds a timer of the same
/// interval starves the feed for ever.
#[tokio::test(start_paused = true)]
async fn a_next_dropped_mid_wait_resumes_rather_than_restarts() {
    let mock = scripted(&[page(&[1]), page(&[2]), page(&[3])]);
    let client = client(&mock, 50).await;
    let mut feed = client
        .watch_room(room(), Since::Newest)
        .await
        .expect("a feed");
    let _ = feed.next().await;

    let mut ticks = tokio::time::interval(Duration::from_secs(2));
    let mut batches = 0;
    for _ in 0..6 {
        tokio::select! {
            batch = feed.next() => {
                if batch.is_ok() {
                    batches += 1;
                }
            }
            _ = ticks.tick() => tokio::time::sleep(Duration::from_millis(1)).await,
        }
    }

    assert!(batches > 0, "the feed never reached a fetch");
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
        .watch_room(room(), Since::Newest)
        .await
        .expect("a feed");

    let mut batches: Vec<_> = feed.into_stream().take(2).collect().await;
    let second = batches.pop().expect("two batches");
    let first = batches.pop().expect("two batches");

    assert_eq!(seqs(first), [1]);
    assert!(matches!(second, Err(ChatError::SessionExpired)));
}

/// A page size of zero would make every page count as full, so nothing would ever wait — and on an
/// idle room `next` would spin without returning.
#[tokio::test(start_paused = true)]
async fn a_page_size_of_zero_still_paces_itself() {
    let mock = scripted(&[page(&[1]), page(&[]), page(&[2])]);
    let client = client(&mock, 0).await;
    let mut feed = client
        .watch_room(room(), Since::Newest)
        .await
        .expect("a feed");
    let _ = feed.next().await;

    let _ = feed.next().await;

    let arrivals = mock.arrivals();
    let gaps: Vec<_> = arrivals[1..]
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect();
    assert!(
        gaps.iter().all(|gap| *gap > Duration::ZERO),
        "every fetch waited: {gaps:?}",
    );
}
