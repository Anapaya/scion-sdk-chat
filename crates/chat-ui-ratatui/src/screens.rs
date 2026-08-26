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
//! The three screens, in the order a launch meets them.
//!
//! Each one draws itself and reads keys, and says what it wants back as an `Intent`. None of them
//! calls a server: [`crate::app`] answers every intent.

pub mod chat;
pub mod connection;
pub mod sign_in;
