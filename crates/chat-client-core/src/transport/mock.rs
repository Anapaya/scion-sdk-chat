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
//! A transport that answers from a script instead of a network.
//!
//! It exists to produce what a real server cannot produce on demand: a body that is not JSON, a 401
//! in the middle of a run, a connection that drops. Always compiled, not just under `cfg(test)`,
//! so that an offline demo can use it too.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use bytes::Bytes;
use http::{Method, Uri};
use tokio::time::Instant;

use super::Transport;
use crate::error::TransportError;

/// A transport whose answers a test writes in advance.
///
/// Cloning shares the script and the recording, so a test keeps a handle to read from after it has
/// handed one to the client.
#[derive(Clone, Default)]
pub struct MockTransport {
    state: Arc<Mutex<MockState>>,
}

#[derive(Default)]
struct MockState {
    /// Answers still to be given, per `METHOD /path`.
    script: HashMap<String, Vec<Answer>>,
    /// Every request that arrived, oldest first.
    received: Vec<Received>,
}

/// One scripted answer.
#[derive(Clone)]
enum Answer {
    /// A reply the server could have sent, valid or not.
    Reply {
        /// The status to answer with.
        status: u16,
        /// The body to answer with, JSON or otherwise.
        body: Bytes,
    },
    /// A request that never completes.
    Fail(TransportError),
}

/// A request the mock was handed, and when.
struct Received {
    /// The head, kept apart from the body because that is what `http` hands over.
    parts: http::request::Parts,
    /// The body as it arrived.
    body: Bytes,
    /// When it arrived, on the clock the test is running — so a paused clock records the gaps the
    /// test set up rather than the microseconds the fetch really took.
    at: Instant,
}

impl MockTransport {
    /// A mock with nothing scripted. Any request reaching it is a mistake in the test.
    pub fn new() -> Self {
        Self::default()
    }

    /// Scripts a reply to `route`, written as `"POST /api/v1/login"`.
    ///
    /// Routes match on method and path only, so a query string belongs in an assertion over
    /// [`last_request`](Self::last_request) rather than here. Scripting the same route again queues
    /// a second answer; the last one stands for every request after the queue runs out, which is
    /// what lets a poll loop run as long as a test needs.
    #[must_use]
    pub fn respond(self, route: &str, status: u16, body: impl Into<Bytes>) -> Self {
        self.script(
            route,
            Answer::Reply {
                status,
                body: body.into(),
            },
        )
    }

    /// Scripts a request to `route` that never completes.
    #[must_use]
    pub fn fail(self, route: &str, error: TransportError) -> Self {
        self.script(route, Answer::Fail(error))
    }

    fn script(self, route: &str, answer: Answer) -> Self {
        self.state
            .lock()
            .expect("a mock is never poisoned")
            .script
            .entry(route.to_owned())
            .or_default()
            .push(answer);

        self
    }

    /// The most recent request, rebuilt as it arrived.
    ///
    /// # Panics
    ///
    /// If nothing has been sent yet.
    pub fn last_request(&self) -> http::Request<Bytes> {
        let state = self.state.lock().expect("a mock is never poisoned");
        let last = state
            .received
            .last()
            .expect("a request to have been sent already");

        http::Request::from_parts(last.parts.clone(), last.body.clone())
    }

    /// How many requests have arrived.
    pub fn request_count(&self) -> usize {
        self.state
            .lock()
            .expect("a mock is never poisoned")
            .received
            .len()
    }

    /// When each request arrived, oldest first, for asserting the gaps between them.
    pub fn arrivals(&self) -> Vec<Instant> {
        self.state
            .lock()
            .expect("a mock is never poisoned")
            .received
            .iter()
            .map(|received| received.at)
            .collect()
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn request(
        &self,
        request: http::Request<Bytes>,
    ) -> Result<http::Response<Bytes>, TransportError> {
        let answer = {
            let mut state = self.state.lock().expect("a mock is never poisoned");
            let route = route_of(request.method(), request.uri());

            let (parts, body) = request.into_parts();
            state.received.push(Received {
                parts,
                body,
                at: Instant::now(),
            });

            state.answer_to(&route)
        };

        match answer {
            Answer::Reply { status, body } => {
                Ok(http::Response::builder()
                    .status(status)
                    .body(body)
                    .expect("a scripted status is valid"))
            }
            Answer::Fail(error) => Err(error),
        }
    }
}

impl MockState {
    /// The next answer for `route`, keeping the last one in place once the queue is down to it.
    ///
    /// # Panics
    ///
    /// If `route` was never scripted. A request to an unscripted route is a test reaching further
    /// than it meant to, and saying so beats answering it with an error the test then explains.
    fn answer_to(&mut self, route: &str) -> Answer {
        let Some(answers) = self.script.get_mut(route) else {
            let mut scripted: Vec<&str> = self.script.keys().map(String::as_str).collect();
            scripted.sort_unstable();

            panic!("nothing is scripted for `{route}`; the script holds {scripted:?}");
        };

        if answers.len() > 1 {
            answers.remove(0)
        } else {
            answers[0].clone()
        }
    }
}

/// The key a route is scripted and looked up under.
fn route_of(method: &Method, uri: &Uri) -> String {
    format!("{method} {}", uri.path())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(uri: &str) -> http::Request<Bytes> {
        http::Request::get(uri)
            .body(Bytes::new())
            .expect("a request")
    }

    #[tokio::test]
    async fn a_scripted_reply_comes_back_as_it_was_written() {
        let mock = MockTransport::new().respond("GET /api/v1/healthz", 200, r#"{"status":"ok"}"#);

        let reply = mock
            .request(get("http://host/api/v1/healthz"))
            .await
            .expect("a reply");

        assert_eq!(reply.status(), 200);
        assert_eq!(reply.body(), &Bytes::from(r#"{"status":"ok"}"#));
    }

    /// The point of the mock: bodies a real server would never send.
    #[tokio::test]
    async fn a_reply_that_is_not_json_is_scriptable() {
        let mock = MockTransport::new().respond("GET /api/v1/rooms", 200, "not json");

        let reply = mock
            .request(get("http://host/api/v1/rooms"))
            .await
            .expect("a reply");

        assert_eq!(reply.body(), &Bytes::from("not json"));
    }

    #[tokio::test]
    async fn a_scripted_failure_never_produces_a_reply() {
        let mock = MockTransport::new().fail("GET /api/v1/rooms", TransportError::Timeout);

        let error = mock
            .request(get("http://host/api/v1/rooms"))
            .await
            .expect_err("a failure");

        assert_eq!(error, TransportError::Timeout);
    }

    /// A queue is consumed in order, so a test can script a run rather than a single answer.
    #[tokio::test]
    async fn queued_answers_are_given_in_the_order_they_were_written() {
        let mock = MockTransport::new()
            .respond("GET /api/v1/rooms", 200, "first")
            .respond("GET /api/v1/rooms", 500, "second");

        for expected in [(200, "first"), (500, "second")] {
            let reply = mock
                .request(get("http://host/api/v1/rooms"))
                .await
                .expect("a reply");

            assert_eq!(
                (reply.status().as_u16(), reply.body()),
                (expected.0, &Bytes::from(expected.1))
            );
        }
    }

    /// Once the queue is down to its last answer it stays there, so a poll loop can run as long as
    /// the test needs without scripting every turn.
    #[tokio::test]
    async fn the_last_answer_stands_for_every_request_after_it() {
        let mock = MockTransport::new().respond("GET /api/v1/rooms", 200, "page");

        for _ in 0..5 {
            let reply = mock
                .request(get("http://host/api/v1/rooms"))
                .await
                .expect("a reply");

            assert_eq!(reply.status(), 200);
        }
        assert_eq!(mock.request_count(), 5);
    }

    /// Routing ignores the query, so a cursor is asserted from the recorded request.
    #[tokio::test]
    async fn a_query_string_does_not_change_which_route_answers() {
        let mock = MockTransport::new().respond("GET /api/v1/rooms/1/messages", 200, "page");

        mock.request(get(
            "http://host/api/v1/rooms/1/messages?after_seq=7&limit=2",
        ))
        .await
        .expect("a reply");

        assert_eq!(
            mock.last_request().uri().query(),
            Some("after_seq=7&limit=2"),
        );
    }

    #[tokio::test]
    async fn what_was_sent_is_recorded_head_and_body() {
        let mock = MockTransport::new().respond("POST /api/v1/rooms", 201, "{}");
        let request = http::Request::post("http://host/api/v1/rooms")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(Bytes::from(r#"{"name":"scion"}"#))
            .expect("a request");

        mock.request(request).await.expect("a reply");

        let recorded = mock.last_request();
        assert_eq!(recorded.method(), Method::POST);
        assert_eq!(recorded.uri().path(), "/api/v1/rooms");
        assert_eq!(
            recorded.headers()[http::header::CONTENT_TYPE],
            "application/json"
        );
        assert_eq!(recorded.body(), &Bytes::from(r#"{"name":"scion"}"#));
    }

    /// Arrival times come off the test's clock, which is what makes a cadence assertion possible
    /// without waiting out the real interval.
    #[tokio::test(start_paused = true)]
    async fn arrivals_are_recorded_on_the_clock_the_test_controls() {
        let mock = MockTransport::new().respond("GET /api/v1/rooms", 200, "page");

        mock.request(get("http://host/api/v1/rooms"))
            .await
            .expect("a reply");
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        mock.request(get("http://host/api/v1/rooms"))
            .await
            .expect("a reply");

        let arrivals = mock.arrivals();
        assert_eq!(arrivals[1] - arrivals[0], std::time::Duration::from_secs(2),);
    }

    #[tokio::test]
    #[should_panic(expected = "nothing is scripted for `GET /api/v1/rooms`")]
    async fn an_unscripted_route_names_itself_rather_than_answering() {
        let mock = MockTransport::new().respond("GET /api/v1/healthz", 200, "{}");

        let _ = mock.request(get("http://host/api/v1/rooms")).await;
    }

    /// A clone shares the script and the recording, which is how a test reads back what the client
    /// it handed the mock to actually sent.
    #[tokio::test]
    async fn a_clone_sees_the_same_script_and_the_same_recording() {
        let mock = MockTransport::new().respond("GET /api/v1/healthz", 200, "{}");
        let handed_over = mock.clone();

        handed_over
            .request(get("http://host/api/v1/healthz"))
            .await
            .expect("a reply");

        assert_eq!(mock.request_count(), 1);
    }
}
