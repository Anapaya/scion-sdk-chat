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
//! What the per-domain test modules share.

use std::fmt;

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

/// Asserts that `value` serializes to exactly `json`, and that `json` deserializes back into
/// `value`.
///
/// Not every type is checked — only the shapes where the schema published by `ToSchema` and the
/// bytes produced by serde could plausibly diverge: an array of objects, a nullable field in both
/// of its states, a nested envelope. Each `json` is copied verbatim from the corresponding type's
/// doc example, so the check doubles as proof that the example is real.
#[track_caller]
pub(super) fn assert_wire_shape<T>(value: T, json: &str)
where
    T: fmt::Debug + PartialEq + Serialize + DeserializeOwned,
{
    let expected: Value = serde_json::from_str(json).expect("the example is valid JSON");
    assert_eq!(
        serde_json::to_value(&value).expect("serializing never fails"),
        expected,
        "serialized shape differs from the doc example"
    );
    assert_eq!(
        serde_json::from_value::<T>(expected).expect("the example decodes"),
        value,
        "the doc example decodes into something else"
    );
}
