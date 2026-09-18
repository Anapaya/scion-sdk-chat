// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.chat

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.LastBaseline
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import com.anapaya.chat.app.ui.theme.chatPalette
import com.anapaya.chat.app.ui.theme.nicknameColour

/** How much of the list's width a bubble may take before it wraps. */
private const val BUBBLE_SHARE = 0.82f

/** A bubble's corners, and the one it flattens towards its neighbour in the same group. */
private val CORNER = 16.dp
private val JOINED = 5.dp

private val CLOCK_SIZE = 10.5.sp

/** What separates the last word from the clock beside it. */
private val CLOCK_GAP = 10.dp

/**
 * The open room's conversation, oldest at the top.
 *
 * Reversed, so index 0 is the newest and stays above the keyboard when it opens.
 */
@Composable
internal fun MessageList(
    rows: List<ChatRow>,
    listState: LazyListState,
    modifier: Modifier = Modifier,
) {
    if (rows.isEmpty()) {
        Empty(modifier)
        return
    }

    BoxWithConstraints(modifier = modifier.fillMaxSize()) {
        val widest = maxWidth * BUBBLE_SHARE

        LazyColumn(
            state = listState,
            reverseLayout = true,
            modifier = Modifier.fillMaxSize(),
            contentPadding = PaddingValues(start = 14.dp, end = 14.dp, top = 16.dp, bottom = 14.dp),
        ) {
            // Without keys every index shifts as a message arrives, and the list jumps.
            items(rows, key = { it.key }, contentType = { it::class }) { row ->
                when (row) {
                    is ChatRow.Day -> DaySeparator(row.label)
                    is ChatRow.Said -> Said(row, widest)
                }
            }
        }
    }
}

/** A bubble's corners: the one facing its neighbour flattens, so a run reads as one block. */
private fun bubbleShape(mine: Boolean, opens: Boolean, closes: Boolean): RoundedCornerShape = when {
    mine && opens -> RoundedCornerShape(CORNER, CORNER, JOINED, CORNER)
    mine && closes -> RoundedCornerShape(CORNER, JOINED, CORNER, CORNER)
    mine -> RoundedCornerShape(CORNER, JOINED, JOINED, CORNER)
    opens -> RoundedCornerShape(CORNER, CORNER, CORNER, JOINED)
    closes -> RoundedCornerShape(JOINED, CORNER, CORNER, CORNER)
    else -> RoundedCornerShape(JOINED, CORNER, CORNER, JOINED)
}

@Composable
private fun Said(row: ChatRow.Said, widest: Dp) {
    val palette = chatPalette

    Column(
        modifier = Modifier
            .fillMaxWidth()
            // A closing run leaves more space than its own rows do. A name under it brings its own.
            .padding(
                bottom = when {
                    !row.closesGroup -> 4.dp
                    row.nameFollows -> 0.dp
                    else -> 10.dp
                },
            ),
        horizontalAlignment = if (row.mine) Alignment.End else Alignment.Start,
    ) {
        // Once per run, and never for the reader: their own side names nobody else.
        if (row.opensGroup && !row.mine) {
            Text(
                text = row.message.username,
                fontSize = 12.sp,
                fontWeight = FontWeight.SemiBold,
                color = nicknameColour(row.message.username),
                modifier = Modifier.padding(top = 12.dp, bottom = 2.dp, start = 4.dp),
            )
        }

        // The clock sits beside the text, so a short message stays short. It is measured first and
        // never shrinks.
        Row(
            modifier = Modifier
                .widthIn(max = widest)
                .background(
                    color = if (row.mine) {
                        palette.ownBubble
                    } else {
                        MaterialTheme.colorScheme.surfaceContainer
                    },
                    shape = bubbleShape(row.mine, row.opensGroup, row.closesGroup),
                )
                .padding(horizontal = 12.dp, vertical = 8.dp),
            verticalAlignment = Alignment.Bottom,
        ) {
            Text(
                text = row.message.body,
                fontSize = 15.sp,
                lineHeight = 15.sp * 1.35f,
                color = if (row.mine) palette.onOwnBubble else MaterialTheme.colorScheme.onSurface,
                modifier = Modifier
                    .weight(1f, fill = false)
                    .alignBy(LastBaseline),
            )

            Text(
                text = row.clock,
                fontSize = CLOCK_SIZE,
                fontFamily = FontFamily.Monospace,
                color = if (row.mine) {
                    palette.onOwnBubble
                } else {
                    MaterialTheme.colorScheme.onSurfaceVariant
                },
                // The last baseline, not the box: the body's line spacing would lift the clock.
                modifier = Modifier
                    .padding(start = CLOCK_GAP)
                    .alignBy(LastBaseline),
            )
        }
    }
}

@Composable
private fun DaySeparator(label: String) {
    Text(
        text = label,
        fontSize = 10.5.sp,
        fontWeight = FontWeight.SemiBold,
        letterSpacing = 0.1.em,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        textAlign = TextAlign.Center,
        modifier = Modifier.fillMaxWidth().padding(top = 18.dp, bottom = 10.dp),
    )
}

/** The way back to the newest message, shown once the reader has left it. Placed by the caller. */
@Composable
internal fun JumpToNewest(onClick: () -> Unit, modifier: Modifier = Modifier) {
    Row(
        modifier = modifier
            .height(34.dp)
            .background(MaterialTheme.colorScheme.primary, RoundedCornerShape(percent = 50))
            .clickable(onClick = onClick)
            .padding(horizontal = 15.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text(
            text = "Newest",
            fontSize = 12.5.sp,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.onPrimary,
        )
        DownArrow(tint = MaterialTheme.colorScheme.onPrimary)
    }
}

@Composable
private fun Empty(modifier: Modifier) {
    Box(modifier = modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text(
            text = "Nothing here yet. Say something.",
            fontSize = 13.5.sp,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
