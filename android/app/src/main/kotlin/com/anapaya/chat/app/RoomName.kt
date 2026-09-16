// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app

import com.anapaya.chat.client.model.Room

/** The longest room name this client will create. The server takes 64. */
public const val ROOM_NAME_MAX: Int = 10

/** Why `name` will not do, or `null` when it will. */
public fun roomNameProblem(name: String): String? = when {
    name.isEmpty() -> "a room needs a name"
    name.any(Char::isWhitespace) -> "a room name is one word"
    name.length > ROOM_NAME_MAX -> "room name is too long - $ROOM_NAME_MAX characters max"
    else -> null
}

/** The newest `seq` the reader has seen in each room. Only the room on screen advances it. */
internal class Unread {
    private val lastSeen = mutableMapOf<Long, Long>()

    /** Marks every room read as it stands, so a session opens quiet. */
    fun seed(rooms: List<Room>) {
        lastSeen.clear()
        rooms.forEach { lastSeen[it.id] = it.latestSeq }
    }

    fun advance(room: Long, seq: Long) {
        if (seq > (lastSeen[room] ?: 0L)) lastSeen[room] = seq
    }

    /** Which rooms hold something unseen. A set: `seq` is server-wide, so a gap counts nothing. */
    fun of(rooms: List<Room>, open: Long?): Set<Long> = buildSet {
        for (room in rooms) {
            if (room.id != open && room.latestSeq > (lastSeen[room.id] ?: 0L)) add(room.id)
        }
    }
}
