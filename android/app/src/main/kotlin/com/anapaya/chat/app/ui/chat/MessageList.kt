// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.chat

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import androidx.compose.foundation.text.InlineTextContent
import androidx.compose.foundation.text.appendInlineContent
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.Placeholder
import androidx.compose.ui.text.PlaceholderVerticalAlign
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import com.anapaya.chat.app.ui.theme.OwnNickname
import com.anapaya.chat.app.ui.theme.chatPalette
import com.anapaya.chat.app.ui.theme.nicknameColour

/** How far a bubble may run before it wraps, so a long line does not span the whole width. */
private val BUBBLE_MAX = 280.dp

/** The corner a bubble turns down, on the side its author is on. */
private val CORNER = 14.dp
private val TAIL = 4.dp

private val CLOCK_SIZE = 10.5.sp

/** What separates the last word from the clock beside it. */
private val CLOCK_GAP = 8.dp

/** Names the gap the clock is drawn into. */
private const val CLOCK_SLOT = "clock"

/**
 * The open room's conversation, oldest at the top.
 *
 * Laid out in reverse so index 0 is the newest message and sits against the composer. That is what
 * keeps the newest message above the keyboard when it opens: a list anchored at its top would slide
 * it underneath.
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

    LazyColumn(
        state = listState,
        reverseLayout = true,
        modifier = modifier.fillMaxSize(),
        contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 4.dp),
    ) {
        // Keys are what let a message arriving while the reader is scrolled up leave the view where
        // it was: without them every index shifts and the list jumps.
        items(rows, key = { it.key }, contentType = { it::class }) { row ->
            when (row) {
                is ChatRow.Day -> DaySeparator(row.label)
                is ChatRow.Said -> Said(row)
            }
        }
    }
}

@Composable
private fun Said(row: ChatRow.Said) {
    val palette = chatPalette

    Column(
        modifier = Modifier
            .fillMaxWidth()
            // A run closes with more space beneath it than separates its own rows, which is what
            // makes it read as one turn rather than several.
            .padding(bottom = if (row.closesGroup) 14.dp else 3.dp),
        horizontalAlignment = if (row.mine) Alignment.End else Alignment.Start,
    ) {
        // Written once per run, and never for the reader: on their own side of the screen there is
        // nobody else it could be.
        if (row.opensGroup && !row.mine) {
            Text(
                text = row.message.username,
                fontSize = 12.sp,
                fontWeight = FontWeight.SemiBold,
                color = nicknameColour(row.message.username),
                modifier = Modifier.padding(start = 2.dp, bottom = 4.dp),
            )
        }

        Box(
            modifier = Modifier
                .widthIn(max = BUBBLE_MAX)
                .background(
                    color = if (row.mine) {
                        palette.ownBubble
                    } else {
                        MaterialTheme.colorScheme.surfaceContainerLowest
                    },
                    shape = if (row.mine) {
                        RoundedCornerShape(CORNER, CORNER, TAIL, CORNER)
                    } else {
                        RoundedCornerShape(CORNER, CORNER, CORNER, TAIL)
                    },
                )
                .then(
                    if (row.mine) {
                        Modifier
                    } else {
                        Modifier.border(
                            width = 1.dp,
                            color = MaterialTheme.colorScheme.outlineVariant,
                            shape = RoundedCornerShape(CORNER, CORNER, CORNER, TAIL),
                        )
                    },
                )
                .padding(horizontal = 13.dp, vertical = 9.dp),
        ) {
            val clockStyle = TextStyle(
                fontSize = CLOCK_SIZE,
                fontFamily = FontFamily.Monospace,
                color = if (row.mine) {
                    palette.onOwnBubble.copy(alpha = 0.7f)
                } else {
                    MaterialTheme.colorScheme.onSurfaceVariant
                },
            )

            // The clock is drawn in the corner, so the text reserves its width at the end of the
            // last line. A message that ends short keeps the clock on that line, and one that fills
            // the line pushes the clock onto the next.
            val measurer = rememberTextMeasurer()
            val density = LocalDensity.current
            val reserved = remember(row.clock, clockStyle, density) {
                with(density) {
                    (measurer.measure(row.clock, clockStyle).size.width.toDp() + CLOCK_GAP).toSp()
                }
            }

            Text(
                text = buildAnnotatedString {
                    append(row.message.body)
                    appendInlineContent(CLOCK_SLOT, " ")
                },
                inlineContent = mapOf(
                    CLOCK_SLOT to InlineTextContent(
                        Placeholder(
                            width = reserved,
                            height = CLOCK_SIZE,
                            placeholderVerticalAlign = PlaceholderVerticalAlign.TextBottom,
                        ),
                    ) {},
                ),
                fontSize = 14.5.sp,
                lineHeight = 14.5.sp * 1.45f,
                color = if (row.mine) palette.onOwnBubble else MaterialTheme.colorScheme.onSurface,
            )

            Text(
                text = row.clock,
                style = clockStyle,
                modifier = Modifier.align(Alignment.BottomEnd),
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

/**
 * The way back to the newest message, shown only once the reader has left it.
 *
 * Placed by the caller, which is the only thing that knows where the bottom of the screen is.
 */
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
