// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.theme

import androidx.compose.ui.graphics.Color

/** The terminal client's nine hues, in its order, darkened to hold 4.5:1 against white. */
private val NICKNAMES = listOf(
    Color(0xFFCF3E69), // pink
    Color(0xFF3C8165), // teal
    Color(0xFF737853), // sage
    Color(0xFF9B5AB7), // lilac
    Color(0xFF3276C7), // blue
    Color(0xFFC132C8), // magenta
    Color(0xFF946880), // mauve
    Color(0xFF7E68B8), // periwinkle
    Color(0xFFBC572F), // salmon
)

/**
 * The colour `name` is drawn in, the same on every client that shows the room.
 *
 * FNV-1a, and unsigned throughout: [String.hashCode] is unspecified across platforms, and a signed
 * remainder would index outside the list.
 */
public fun nicknameColour(name: String): Color {
    var hash = 0xcbf29ce484222325uL

    for (byte in name.toByteArray(Charsets.UTF_8)) {
        hash = hash xor byte.toUByte().toULong()
        hash *= 0x100000001b3uL
    }

    return NICKNAMES[(hash % NICKNAMES.size.toULong()).toInt()]
}
