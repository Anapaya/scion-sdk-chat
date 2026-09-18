// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.chat

import com.anapaya.chat.client.model.Message
import java.text.SimpleDateFormat
import java.util.Calendar
import java.util.Date
import java.util.Locale

/** A row of the conversation: something somebody said, or the day it was said on. */
internal sealed interface ChatRow {
    val key: String

    data class Day(val label: String) : ChatRow {
        override val key: String get() = "day $label"
    }

    /** One message, and where it sits in its author's run. */
    data class Said(
        val message: Message,
        val clock: String,
        val mine: Boolean,
        val opensGroup: Boolean,
        val closesGroup: Boolean,
        /** A name under this message carries the gap, so the row leaves none of its own. */
        val nameFollows: Boolean,
    ) : ChatRow {
        override val key: String get() = "seq ${message.seq}"
    }
}

/** The clock and the date. [Locale.ROOT] keeps it at 24 hours, as the terminal client prints it. */
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

    /** What to write above the day's first message. */
    fun label(at: Long): String {
        val today = dayOf(now())
        return when (dayOf(at)) {
            today -> "TODAY"
            today - 1 -> "YESTERDAY"
            else -> date.format(Date(at)).uppercase(Locale.ROOT)
        }
    }
}

/** The messages as rows, oldest first: the only order a change of day or of author is visible in. */
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
        // A day separator breaks a run, so the name is written again under it.
        val opens = newDay || before?.username != message.username
        val closes = after == null ||
            after.username != message.username ||
            clocks.dayOf(after.postedAt) != on

        // A day separator carries its own space, so only a name on the same day counts.
        val nameFollows = closes &&
            after != null &&
            after.username != me &&
            clocks.dayOf(after.postedAt) == on

        rows.add(
            ChatRow.Said(
                message = message,
                clock = clocks.clock(message.postedAt),
                mine = message.username == me,
                opensGroup = opens,
                closesGroup = closes,
                nameFollows = nameFollows,
            ),
        )
    }

    return rows
}
