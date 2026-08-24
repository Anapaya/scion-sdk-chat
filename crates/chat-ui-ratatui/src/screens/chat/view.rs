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
//! How the chat screen is drawn: the sidebar, the message pane, the line being typed.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, List, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

use super::{Chat, commands};
use crate::ui::{self, NAME_WIDTH, field, theme};

/// How wide the room list is. Fixed, because a room name is short and the messages want the rest.
const SIDEBAR_WIDTH: u16 = 18;

impl Chat {
    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let [sidebar, opened] =
            Layout::horizontal([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(20)])
                .areas(area);
        // The error line is given no row at all when there is nothing to report, so the message
        // pane grows into it rather than the screen carrying a blank row.
        let [messages, composer, error] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Length(u16::from(self.error.is_some())),
        ])
        .areas(opened);

        self.draw_rooms(frame, sidebar);
        let measured = self.draw_messages(frame, messages);
        self.measured = measured;
        field::draw(
            frame,
            composer,
            commands::instructions(),
            &self.input,
            true,
            false,
        );
        ui::draw_error(frame, error, self.error.as_deref());
    }

    fn draw_rooms(&mut self, frame: &mut Frame, area: Rect) {
        let names = self.rooms.iter().map(|room| {
            if self.unread(room) {
                Line::from(vec![
                    Span::from(format!("#{}", room.name))
                        .fg(theme::UNREAD)
                        .bold(),
                    Span::from(" ●").fg(theme::UNREAD),
                ])
            } else {
                Line::from(Span::from(format!("#{}", room.name)).fg(theme::TEXT))
            }
        });
        let list = List::new(names)
            .block(panel(" Rooms "))
            .highlight_symbol("▸ ")
            .highlight_style(
                Style::new()
                    .bg(theme::SELECTION)
                    .fg(theme::HIGHLIGHT)
                    .add_modifier(Modifier::BOLD),
            );

        frame.render_stateful_widget(list, area, &mut self.open);
    }

    /// Draws the messages, and reports the largest offset and how many rows fit.
    fn draw_messages(&self, frame: &mut Frame, area: Rect) -> (u16, u16) {
        let title = match self.open_room() {
            Some(room) => format!(" #{} ", room.name),
            None => " no rooms ".to_owned(),
        };
        let mut lines: Vec<Line<'_>> = self
            .messages
            .iter()
            .map(|message| {
                Line::from(vec![
                    Span::from(format!("{:>NAME_WIDTH$} ", message.username))
                        .fg(self.colour(&message.username)),
                    Span::from(message.body.as_str()).fg(theme::TEXT),
                ])
            })
            .collect();
        lines.extend(self.notices.iter().cloned());

        let block = panel(&title);
        let inner = block.inner(area);
        let messages = Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .block(block);

        // A message longer than the pane takes several rows, so the newest is found by counting the
        // rows a wrap actually produces rather than the messages. The count includes the block's
        // own two rows, so it is compared against the height that also has them.
        let rows = messages.line_count(inner.width) as u16;
        let bottom = rows.saturating_sub(area.height);
        // Following the newest until the reader says otherwise, and never past the end when the
        // pane has grown a message since they last looked.
        let scroll = self.scroll.unwrap_or(bottom).min(bottom);

        frame.render_widget(messages.scroll((scroll, 0)), area);
        draw_scrollbar(frame, area, inner.height, scroll, bottom);

        (bottom, inner.height)
    }
}

/// Draws how far down the pane is, on its own right border.
///
/// `offset` is a count of rows scrolled past and `bottom` the largest it reaches, so `bottom + 1`
/// is how many positions there are and `visible` is how many of them a thumb covers. Nothing is
/// drawn for a pane that fits, where a thumb would fill the track and say nothing.
fn draw_scrollbar(frame: &mut Frame, area: Rect, visible: u16, offset: u16, bottom: u16) {
    if bottom == 0 {
        return;
    }

    let mut state = ScrollbarState::new(bottom as usize + 1)
        .position(offset as usize)
        .viewport_content_length(visible as usize);

    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            // The arrow heads would land on the rounded corners, so the track is inset past them
            // and drawn without heads instead.
            .begin_symbol(None)
            .end_symbol(None)
            .track_style(Style::new().fg(theme::BORDER))
            .thumb_style(Style::new().fg(theme::FOCUS)),
        area.inner(Margin::new(0, 1)),
        &mut state,
    );
}

/// A panel: named in the colour every screen gives a title, and filled a shade lighter than the
/// screen behind it.
fn panel(title: &str) -> Block<'_> {
    ui::bordered(
        Line::from(Span::from(title).fg(theme::TITLE).bold()),
        theme::BORDER,
        theme::PANEL,
    )
}
