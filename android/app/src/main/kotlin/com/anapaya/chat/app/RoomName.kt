// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app

import com.anapaya.chat.client.model.Room

/**
 * The longest room name this client will create.
 *
 * The server takes 64. This is the terminal client's limit, whose sidebar is 18 columns wide, and
 * it is kept here so a room made on a phone reads in full there rather than clipped. Names from
 * anywhere else are still shown at whatever length they arrive.
 */
public const val ROOM_NAME_MAX: Int = 10

/**
 * Why `name` will not do, or `null` when it will.
 *
 * Answered here rather than by the server, so a name that cannot work is refused without a round
 * trip and the reason names the rule rather than the status code.
 */
public fun roomNameProblem(name: String): String? = when {
    name.isEmpty() -> "a room needs a name"
    name.any(Char::isWhitespace) -> "a room name is one word"
    name.length > ROOM_NAME_MAX -> "room name is too long - $ROOM_NAME_MAX characters max"
    else -> null
}

/**
 * The newest `seq` the reader has actually seen in each room.
 *
 * Only the room on screen advances it, which is what makes it a badge cursor rather than a resume
 * cursor: the feed's own cursor moves whether anyone is looking or not.
 */
internal class Unread {
    private val lastSeen = mutableMapOf<Long, Long>()

    /**
     * Marks every room read as it stands, so a session opens quiet rather than claiming all of
     * history is new. Replaces rather than merges, so signing in again does not inherit the last
     * session's cursors.
     */
    fun seed(rooms: List<Room>) {
        lastSeen.clear()
        rooms.forEach { lastSeen[it.id] = it.latestSeq }
    }

    fun advance(room: Long, seq: Long) {
        if (seq > (lastSeen[room] ?: 0L)) lastSeen[room] = seq
    }

    /**
     * Which rooms hold something unseen.
     *
     * A set, never a count: `seq` is assigned server-wide, so the gap between two of them spans
     * other rooms' messages and counts nothing a reader would recognise.
     *
     * `open` is excluded outright rather than through its cursor, so the room on screen is never
     * marked unread while its first batch is still on the way.
     */
    fun of(rooms: List<Room>, open: Long?): Set<Long> = buildSet {
        for (room in rooms) {
            if (room.id != open && room.latestSeq > (lastSeen[room.id] ?: 0L)) add(room.id)
        }
    }
}
