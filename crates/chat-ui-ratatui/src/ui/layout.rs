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
//! Where a form sits in a terminal that is any size at all.

use ratatui::layout::{Constraint, Flex, Layout, Rect};

/// How wide a form is drawn, whatever room the terminal gives it.
const FORM_WIDTH: u16 = 60;

/// The column a form sits in: [`FORM_WIDTH`] centred in `area`, or all of it when the terminal is
/// narrower.
pub fn form(area: Rect) -> Rect {
    let [centred] = Layout::horizontal([Constraint::Max(FORM_WIDTH)])
        .flex(Flex::Center)
        .areas(area);

    centred
}
