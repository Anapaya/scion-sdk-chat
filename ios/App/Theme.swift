// Copyright 2026 Anapaya Systems

import SwiftUI

/// The roles the design names.
///
/// Blue carries emphasis — the open room, the reader's own messages, anything unread. Orange is
/// kept for risk. Green says the transport is carrying.
enum Palette {
    static let accent = Color(hex: 0x009CDE)
    static let strongBlue = Color(hex: 0x165A97)
    static let ink = Color(hex: 0x1C1F2A)
    static let dim = Color(hex: 0x5A6472)
    static let hairline = Color(hex: 0xE6EBF0)
    static let fieldFill = Color(hex: 0xF1F4F8)
    static let fieldBorder = Color(hex: 0xE0E6ED)
    static let sendDisabled = Color(hex: 0xB8C2CC)
    static let live = Color(hex: 0x6DBE45)
    static let risk = Color(hex: 0xFF871F)

    /// Behind a message the reader wrote, and the ink on it. White on that blue reaches 3.1:1,
    /// below the 4.5:1 the small text asks for.
    static let ownBubble = accent
    static let onOwnBubble = Color.white
}

/// The nine hues a name is drawn in, dark enough to hold 4.5:1 against white at 12pt.
private let nicknames: [Color] = [
    Color(hex: 0xCF3E69),
    Color(hex: 0x3C8165),
    Color(hex: 0x737853),
    Color(hex: 0x9B5AB7),
    Color(hex: 0x3276C7),
    Color(hex: 0xC132C8),
    Color(hex: 0x946880),
    Color(hex: 0x7E68B8),
    Color(hex: 0xBC572F),
]

/**
 The colour `name` is drawn in, the same on every client that shows the room.

 FNV-1a, which is specified, so two clients agree. Unsigned throughout: a signed remainder would
 index outside the list.
 */
func nicknameColour(_ name: String) -> Color {
    var hash: UInt64 = 0xcbf2_9ce4_8422_2325
    for byte in Array(name.utf8) {
        hash ^= UInt64(byte)
        hash = hash &* 0x0000_0100_0000_01b3
    }
    return nicknames[Int(hash % UInt64(nicknames.count))]
}

extension Color {
    init(hex: UInt32) {
        self.init(
            .sRGB,
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255)
    }
}
