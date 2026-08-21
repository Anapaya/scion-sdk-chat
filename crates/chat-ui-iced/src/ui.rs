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
//! The one widget all three screens share.

use iced::{
    Element,
    widget::{Space, text},
};

use crate::app::Message;

/// The error line, or nothing at all.
///
/// A `Space` rather than an absent widget, so showing an error does not move the form up the
/// screen.
pub fn error_line(error: Option<&str>) -> Element<'_, Message> {
    match error {
        Some(error) => {
            text(format!("⚠ {error}"))
                .style(text::danger)
                .size(14)
                .into()
        }
        None => Space::new().height(20).into(),
    }
}
