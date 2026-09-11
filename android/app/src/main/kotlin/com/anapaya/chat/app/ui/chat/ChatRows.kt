// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.chat

import com.anapaya.chat.client.model.Message
import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Date
import java.util.Locale

/**
 * A row of the conversation: something somebody said, or the day it was said on.
 *
 * Days are rows rather than sticky headers because the list is laid out newest-first, where a
 * sticky header would pin to the oldest end.
 */
internal sealed interface ChatRow {
    /** What keeps a row where it was when rows arrive beside it. */
    val key: String

    data class Day(val label: String) : ChatRow {
        override val key: String get() = "day $label"
    }

    /**
     * One message, and where it sits in its author's run.
     *
     * Consecutive messages by one person are drawn as a group: the name is written once at the top
     * of the run, and the run is separated from the next by more space than its own rows are from
     * each other.
     */
    data class Said(
        val message: Message,
        val clock: String,
        val mine: Boolean,
        val opensGroup: Boolean,
        val closesGroup: Boolean,
    ) : ChatRow {
        override val key: String get() = "seq ${message.seq}"
    }
}

/**
 * The clock and the date, fixed rather than the reader's.
 *
 * [Locale.ROOT] because the terminal client prints a literal 24-hour clock, and a locale that reads
 * 12-hour would have the two disagree about the same message. `java.time` would say this more
 * clearly but arrives at API 26, and this app runs from 24.
 */
internal class Clocks(private val now: () -> Long = System::currentTimeMillis) {
    private val time = SimpleDateFormat("HH:mm", Locale.ROOT)
    private val date = SimpleDateFormat("d MMM", Locale.ROOT)
    private val on = Calendar.getInstance()

    fun clock(at: Long): String = time.format(Date(at))

    /** Which day `at` falls on locally, as one comparable number. */
    fun dayOf(at: Long): Long {
        on.timeInMillis = at
        return on.get(Calendar.YEAR) * 1000L + on.get(Calendar.DAY_OF_YEAR)
    }

    /** What to write above the day's first message. Named where a name is shorter than a date. */
    fun label(at: Long): String {
        val today = dayOf(now())
        return when (dayOf(at)) {
            today -> "TODAY"
            today - 1 -> "YESTERDAY"
            else -> date.format(Date(at)).uppercase(Locale.ROOT)
        }
    }
}

/**
 * The messages as rows, oldest first, with a separator wherever the local date changes and each
 * message told where it sits in its author's run.
 *
 * Built oldest-first because that is the only order in which a change of day, or of author, can be
 * seen. The list is drawn reversed, which puts a day's separator above the messages it introduces.
 */
internal fun chatRows(messages: List<Message>, me: String?, clocks: Clocks): List<ChatRow> {
    val rows = ArrayList<ChatRow>(messages.size + 2)
    var day: Long? = null

    messages.forEachIndexed { index, message ->
        val on = clocks.dayOf(message.postedAt)
        val newDay = on != day
        if (newDay) {
            day = on
            rows.add(ChatRow.Day(clocks.label(message.postedAt)))
        }

        val before = messages.getOrNull(index - 1)
        val after = messages.getOrNull(index + 1)
        // A day separator breaks a run: a name is written again under it, or the group would read
        // as continuing across a gap of hours.
        val opens = newDay || before?.username != message.username
        val closes = after == null ||
            after.username != message.username ||
            clocks.dayOf(after.postedAt) != on

        rows.add(
            ChatRow.Said(
                message = message,
                clock = clocks.clock(message.postedAt),
                mine = message.username == me,
                opensGroup = opens,
                closesGroup = closes,
            ),
        )
    }

    return rows
}
