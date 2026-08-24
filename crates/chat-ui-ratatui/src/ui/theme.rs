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
//! Ferra, the palette Halloy opens with.
//!
//! Taken from <https://github.com/squidowl/halloy/blob/main/assets/themes/ferra.toml>. The colours
//! are given as RGB, so the terminal's own palette does not apply and every screen looks the same
//! whatever the user's theme.

use ratatui::style::Color;

/// Behind everything.
pub const BACKGROUND: Color = Color::Rgb(0x2b, 0x29, 0x2d);

/// Behind a panel that holds content.
pub const PANEL: Color = Color::Rgb(0x24, 0x22, 0x26);

/// Behind a line being typed into, a shade darker than a panel.
pub const INPUT: Color = Color::Rgb(0x20, 0x1e, 0x21);

/// A border nobody is typing into.
pub const BORDER: Color = Color::Rgb(0x4f, 0x47, 0x4d);

/// The border, and the keys, of whatever has focus.
pub const FOCUS: Color = Color::Rgb(0x7a, 0xae, 0xdc);

/// Message bodies, room names, what is being typed.
pub const TEXT: Color = Color::Rgb(0xfe, 0xcd, 0xb2);

/// Labels beside a key, and anything the eye should skip.
pub const DIM: Color = Color::Rgb(0xab, 0x8a, 0x79);

/// When a message was posted. Grey rather than of the palette, so it reads as a note on the row
/// instead of another voice in it.
pub const TIMESTAMP: Color = Color::Rgb(0x7d, 0x77, 0x7b);

/// The name of a panel.
pub const TITLE: Color = Color::Rgb(0xd7, 0xbd, 0xe2);

/// Behind the room that is open.
pub const SELECTION: Color = Color::Rgb(0x45, 0x3d, 0x41);

/// The room that is open.
pub const HIGHLIGHT: Color = Color::Rgb(0xf5, 0xd7, 0x6e);

/// Something went wrong.
pub const ERROR: Color = Color::Rgb(0xe0, 0x6b, 0x75);

/// A room holding messages nobody has read. Ferra names this one for the purpose.
pub const UNREAD: Color = Color::Rgb(0xff, 0xa0, 0x7a);

/// Whoever is logged in, so their own lines are found at a glance.
pub const OWN_NICKNAME: Color = Color::Rgb(0xf5, 0xd7, 0x6e);

/// What everyone else's name is drawn in, picked by [`nickname`].
const NICKNAMES: [Color; 9] = [
    Color::Rgb(0xf6, 0xb6, 0xc9), // pink
    Color::Rgb(0x9c, 0xd1, 0xbb), // teal
    Color::Rgb(0xb1, 0xb6, 0x95), // sage
    Color::Rgb(0xd7, 0xbd, 0xe2), // lilac
    Color::Rgb(0x4a, 0x90, 0xe2), // blue
    Color::Rgb(0xd2, 0x53, 0xd8), // magenta
    Color::Rgb(0xaf, 0x8d, 0x9f), // mauve
    Color::Rgb(0xb4, 0xa7, 0xd6), // periwinkle
    Color::Rgb(0xff, 0xa0, 0x7a), // salmon
];

/// The colour `name` is drawn in, the same on every client that shows the room.
///
/// FNV-1a rather than the hasher behind `HashMap`, which is seeded per process: two clients would
/// then colour the same person differently.
pub fn nickname(name: &str) -> Color {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;

    for byte in name.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    NICKNAMES[(hash % NICKNAMES.len() as u64) as usize]
}
