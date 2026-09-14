// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.theme

import androidx.compose.ui.graphics.Color

/**
 * What everyone else's name is drawn in.
 *
 * The terminal client's nine `NICKNAMES` hues, in its order, darkened to hold 4.5:1 against white.
 * Its palette is picked for a dark terminal, where the light originals carry; here a name is 12sp on
 * a white page. The hue of each is kept, so one person reads as the same colour in both clients.
 */
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
 * FNV-1a rather than [String.hashCode], which is neither specified across platforms nor the hash
 * the terminal client uses: the two would colour the same person differently.
 *
 * Unsigned throughout. [Byte.toLong] sign-extends, which would poison the hash for any byte over
 * 0x7f, and the remainder of a negative [Long] is negative, which would index outside the list.
 */
public fun nicknameColour(name: String): Color {
    var hash = 0xcbf29ce484222325uL

    for (byte in name.toByteArray(Charsets.UTF_8)) {
        hash = hash xor byte.toUByte().toULong()
        // ULong wraps, which is what the terminal client's wrapping_mul does.
        hash *= 0x100000001b3uL
    }

    return NICKNAMES[(hash % NICKNAMES.size.toULong()).toInt()]
}
