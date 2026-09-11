// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.theme

import androidx.compose.ui.graphics.Color

/**
 * What everyone else's name is drawn in.
 *
 * The same nine colours, in the same order, as the terminal client's `NICKNAMES`. Both clients pick
 * from this list with the same hash, so one person is one colour wherever the room is read.
 */
private val NICKNAMES = listOf(
    Color(0xFFF6B6C9), // pink
    Color(0xFF9CD1BB), // teal
    Color(0xFFB1B695), // sage
    Color(0xFFD7BDE2), // lilac
    Color(0xFF4A90E2), // blue
    Color(0xFFD253D8), // magenta
    Color(0xFFAF8D9F), // mauve
    Color(0xFFB4A7D6), // periwinkle
    Color(0xFFFFA07A), // salmon
)

/** Whoever is logged in, so their own lines are found at a glance. */
public val OwnNickname: Color = Color(0xFFF5D76E)

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
