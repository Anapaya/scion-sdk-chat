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
//! One text field, drawn the same way on every screen.

use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::Stylize,
    text::Span,
    widgets::{Block, Paragraph},
};
use tui_input::Input;

/// What a masked field shows instead of what was typed.
const MASK: char = '•';

/// Draws a bordered field, and on the focused one puts the terminal's own cursor where the next
/// character will land.
///
/// The value is scrolled here rather than by the input, because what fits depends on how wide the
/// box is and nothing knows that until it is being drawn.
pub fn draw(frame: &mut Frame, area: Rect, title: &str, input: &Input, focused: bool, mask: bool) {
    let block = if focused {
        Block::bordered().title(title.bold()).blue()
    } else {
        Block::bordered().title(Span::from(title))
    };
    let inner = block.inner(area);

    // A mask stands one character per character typed, so the cursor still lands in the right
    // place.
    let shown = if mask {
        MASK.to_string().repeat(input.value().chars().count())
    } else {
        input.value().to_owned()
    };
    let scroll = input.visual_scroll(inner.width as usize);

    frame.render_widget(
        Paragraph::new(shown)
            .scroll((0, scroll as u16))
            .block(block),
        area,
    );

    if focused {
        let cursor = input.visual_cursor().saturating_sub(scroll);
        frame.set_cursor_position(Position::new(inner.x + cursor as u16, inner.y));
    }
}
