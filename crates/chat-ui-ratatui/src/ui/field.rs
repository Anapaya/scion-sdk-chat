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
    text::{Line, Span},
    widgets::Paragraph,
};
use tui_input::Input;

use crate::ui::{self, theme};

/// What a masked field shows instead of what was typed.
const MASK: char = '•';

/// How a field is drawn, beyond the value in it.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// The keys are going here.
    Focused,
    /// On screen, and Tab reaches it.
    Idle,
    /// On screen, holding its value, but nothing this screen is doing can use it.
    Disabled,
}

impl State {
    /// For a screen whose fields are only ever focused or not.
    pub fn focused(yes: bool) -> Self {
        if yes { Self::Focused } else { Self::Idle }
    }
}

/// Draws a bordered field, and on the focused one puts the terminal's own cursor where the next
/// character will land.
///
/// The value is scrolled here rather than by the input, because what fits depends on how wide the
/// box is and nothing knows that until it is being drawn.
pub fn draw(
    frame: &mut Frame,
    area: Rect,
    title: Line<'_>,
    input: &Input,
    state: State,
    mask: bool,
) {
    let focused = state == State::Focused;
    let border = match state {
        State::Focused => theme::FOCUS,
        State::Idle => theme::BORDER,
        State::Disabled => theme::SELECTION,
    };
    let text = if state == State::Disabled {
        theme::SELECTION
    } else {
        theme::TEXT
    };
    let block = ui::bordered(title, border, theme::INPUT);
    let inner = block.inner(area);

    // One mask per column, not per character: the cursor and the scroll are both measured in
    // columns, and a character wider than one would put them past what is drawn.
    let shown = if mask {
        MASK.to_string().repeat(Span::from(input.value()).width())
    } else {
        input.value().to_owned()
    };
    let scroll = input.visual_scroll(inner.width as usize);

    frame.render_widget(
        Paragraph::new(shown)
            .fg(text)
            .scroll((0, scroll as u16))
            .block(block),
        area,
    );

    if focused {
        let cursor = input.visual_cursor().saturating_sub(scroll);
        frame.set_cursor_position(Position::new(inner.x + cursor as u16, inner.y));
    }
}

/// Draws a bordered choice, one option per column, with the chosen one lit.
///
/// The same box as [`draw`] so the two sit in a form together, but nothing is typed into it and no
/// cursor is placed: the options are all there is, and one of them is always chosen.
pub fn choice(
    frame: &mut Frame,
    area: Rect,
    title: Line<'_>,
    options: &[(&str, bool)],
    focused: bool,
) {
    let border = if focused { theme::FOCUS } else { theme::BORDER };
    let block = ui::bordered(title, border, theme::INPUT);

    let mut spans = Vec::with_capacity(options.len() * 2);
    for (label, chosen) in options {
        spans.push(Span::from(" "));
        spans.push(if *chosen {
            Span::from(format!("[{label}]")).fg(theme::HIGHLIGHT).bold()
        } else {
            Span::from(format!(" {label} ")).fg(theme::DIM)
        });
    }

    frame.render_widget(Paragraph::new(Line::from(spans)).block(block), area);
}
