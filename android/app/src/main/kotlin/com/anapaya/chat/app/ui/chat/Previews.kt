// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.chat

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Surface
import androidx.compose.material3.windowsizeclass.WindowWidthSizeClass
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import com.anapaya.chat.app.Screen
import com.anapaya.chat.app.UiState
import com.anapaya.chat.app.ui.theme.ChatTheme
import com.anapaya.chat.client.model.Message
import com.anapaya.chat.client.model.Room

/** Night mode, for a preview that should be drawn in the dark scheme. */
private const val NIGHT = 0x21

private val ROOMS = listOf(
    Room(id = 1, latestSeq = 12, name = "lobby"),
    Room(id = 2, latestSeq = 40, name = "general"),
    Room(id = 3, latestSeq = 7, name = "scion"),
)

/** Fixed, so a preview does not change with the clock. Sits mid-afternoon. */
private const val NOON = 1_788_000_000_000L

/**
 * A run each way, so grouping, the author colours and both bubble shapes are all on screen.
 *
 * `ada` is the reader. The names are different lengths and hash to different colours, which is the
 * one thing here that cannot be checked without looking.
 */
private val MESSAGES = listOf(
    Message(seq = 8, username = "grace", body = "the tunnel came up", postedAt = NOON),
    Message(seq = 9, username = "grace", body = "on both ASes", postedAt = NOON + 20_000),
    Message(seq = 10, username = "ada", body = "emulator and terminal?", postedAt = NOON + 60_000),
    Message(
        seq = 11,
        username = "ada",
        body = "a longer line, so the bubble has to wrap and the clock has to sit under it " +
            "rather than run off the end",
        postedAt = NOON + 90_000,
    ),
    Message(seq = 12, username = "linus", body = "same room, both of them", postedAt = NOON + 120_000),
)

private fun state(error: String? = null, pending: Boolean = false) = UiState(
    screen = Screen.Chat,
    rooms = ROOMS,
    openRoomId = 1,
    messages = MESSAGES,
    unread = setOf(2L),
    username = "ada",
    pending = pending,
    actionError = error,
)

@Composable
private fun Framed(content: @Composable () -> Unit) {
    ChatTheme { Surface { content() } }
}

@Preview(name = "Phone", showBackground = true, widthDp = 412, heightDp = 892)
@Preview(name = "Phone, dark", showBackground = true, widthDp = 412, heightDp = 892, uiMode = NIGHT)
@Composable
private fun PhonePreview() {
    Framed {
        ChatScreen(
            state = state(),
            width = WindowWidthSizeClass.Compact,
            onOpenRoom = {},
            onSend = {},
            onCreateRoom = {},
            onDraftRestored = {},
        )
    }
}

/** The banner and the spinner together, which is what a failed send during a slow call looks like. */
@Preview(name = "Failed send", showBackground = true, widthDp = 412, heightDp = 892)
@Composable
private fun FailurePreview() {
    Framed {
        ChatScreen(
            state = state(error = "Send failed: no reply from the control URL.", pending = true),
            width = WindowWidthSizeClass.Compact,
            onOpenRoom = {},
            onSend = {},
            onCreateRoom = {},
            onDraftRestored = {},
        )
    }
}

/** The pinned layout, which otherwise needs a tablet to see at all. */
@Preview(name = "Pinned rooms", showBackground = true, widthDp = 1000, heightDp = 700)
@Composable
private fun PinnedPreview() {
    Framed {
        ChatScreen(
            state = state(),
            width = WindowWidthSizeClass.Expanded,
            onOpenRoom = {},
            onSend = {},
            onCreateRoom = {},
            onDraftRestored = {},
        )
    }
}

@Preview(name = "Rooms", showBackground = true, widthDp = 296)
@Preview(name = "Rooms, dark", showBackground = true, widthDp = 296, uiMode = NIGHT)
@Composable
private fun RoomListPreview() {
    Framed {
        RoomList(
            rooms = ROOMS,
            openRoomId = 1,
            unread = setOf(2L),
            pending = false,
            onOpenRoom = {},
            onCreate = {},
        )
    }
}

@Preview(name = "Messages", showBackground = true, widthDp = 412, heightDp = 420)
@Preview(name = "Messages, dark", showBackground = true, widthDp = 412, heightDp = 420, uiMode = NIGHT)
@Composable
private fun MessageListPreview() {
    Framed {
        val clocks = remember { Clocks() }
        MessageList(
            rows = remember { chatRows(MESSAGES, "ada", clocks).asReversed() },
            listState = rememberLazyListState(),
            modifier = Modifier.fillMaxSize(),
        )
    }
}

@Preview(name = "Composer", showBackground = true, widthDp = 412)
@Composable
private fun ComposerPreview() {
    Framed {
        Composer(draft = "both ends of the tunnel", room = "lobby", pending = false, onDraft = {}, onSend = {})
    }
}

@Preview(name = "Composer, sending", showBackground = true, widthDp = 412)
@Composable
private fun ComposerPendingPreview() {
    Framed {
        Composer(draft = "one moment", room = "lobby", pending = true, onDraft = {}, onSend = {})
    }
}

@Preview(name = "New room", showBackground = true, widthDp = 412, heightDp = 320)
@Composable
private fun CreateRoomPreview() {
    Framed { CreateRoomDialog(pending = false, onDismiss = {}, onCreate = {}) }
}
