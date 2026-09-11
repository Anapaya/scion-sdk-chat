// Copyright 2026 Anapaya Systems

package com.anapaya.chat.client

import com.anapaya.chat.client.model.Message
import com.anapaya.chat.client.model.Room
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow

/** How often the open room is re-read, matching the terminal client. */
private const val MESSAGES_INTERVAL_MILLIS = 1_000L

/** The room list is less urgent than the open room. */
private const val ROOMS_INTERVAL_MILLIS = 2_000L

/**
 * Everything posted to `room`, as it arrives.
 *
 * Emits what is already there, then only what is new: `after` the newest `seq` it has seen, so a
 * long-running room is not re-read every second.
 *
 * The delay comes after each answer rather than on a fixed tick. A reply slower than the interval
 * would otherwise queue the next request behind it, and a stall would arrive as a burst.
 */
public fun ChatClient.roomMessages(room: Long): Flow<List<Message>> = flow {
    var newest = 0L

    while (true) {
        val batch = when (newest) {
            0L -> messagesNewest(room)
            else -> messagesAfter(room, newest)
        }

        if (batch.isNotEmpty()) {
            newest = batch.maxOf { it.seq }
            emit(batch)
        }

        delay(MESSAGES_INTERVAL_MILLIS)
    }
}

/** The rooms, re-read until the collector stops. */
public fun ChatClient.roomList(): Flow<List<Room>> = flow {
    while (true) {
        emit(rooms())
        delay(ROOMS_INTERVAL_MILLIS)
    }
}
