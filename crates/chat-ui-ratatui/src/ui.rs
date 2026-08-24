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
//! What every screen is drawn from: one field, one box, one palette.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::Line,
    widgets::{Block, BorderType, Paragraph},
};

pub mod field;
pub mod layout;
pub mod theme;

/// How much room a name is given before the message it sent.
///
/// Also the longest name this client will register, since a wider one would push its own messages
/// out of line with every other row. The server accepts more; this is the limit of what the pane
/// can draw tidily.
pub const NAME_WIDTH: usize = 10;

/// The box a panel and a field are both drawn in: rounded, named, and filled a shade apart from the
/// screen behind it.
///
/// One place decides that shape, so the message pane and the line being typed cannot drift into
/// looking like two different applications.
pub fn bordered<'a>(title: impl Into<Line<'a>>, border: Color, fill: Color) -> Block<'a> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border))
        .title(title.into())
        .style(Style::new().bg(fill))
}

/// The name of a field, in the colour every screen gives one.
pub fn label(text: &str) -> Line<'_> {
    Line::from(text.fg(theme::TITLE))
}

/// A complaint, in the shape every screen makes one: the mark, then what went wrong.
///
/// Returned rather than drawn, because a complaint about a typed line is written into the message
/// pane while a failed call is drawn on a row of its own.
pub fn warning(message: &str) -> Line<'static> {
    Line::from(format!("⚠ {message}")).fg(theme::ERROR)
}

/// Draws why the last attempt failed, and nothing at all when none has.
pub fn draw_error(frame: &mut Frame, area: Rect, error: Option<&str>) {
    if let Some(message) = error {
        frame.render_widget(Paragraph::new(warning(message)), area);
    }
}
