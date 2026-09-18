// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.chat

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.anapaya.chat.app.ui.theme.chatPalette

private val PILL = RoundedCornerShape(percent = 50)

/** The send button. */
private val BUTTON = 42.dp

private val FIELD = RoundedCornerShape(18.dp)
private val FIELD_BORDER = 1.dp
private val FIELD_PAD = 11.dp

/** [FIELD] plus the ring's offset, so the two curves stay parallel. */
private val RING = RoundedCornerShape(20.dp)

/** How wide the focus ring is, and how far outside the field it sits. */
private val RING_WIDTH = 2.dp
private val RING_GAP = 2.dp

/** One line of the field, at 15sp and the line height the bubbles use. */
private val LINE = 20.dp

/** Lifts the button until its middle is on the field's last line, clear of the field's own inset. */
private val BUTTON_LIFT = RING_GAP + FIELD_BORDER + FIELD_PAD - (BUTTON - LINE) / 2

/** How far the field grows before it scrolls instead. */
private const val MAX_LINES = 3

/** The line being typed, and the way to post it. The send button carries the spinner. */
@Composable
internal fun Composer(
    draft: String,
    room: String?,
    pending: Boolean,
    onDraft: (String) -> Unit,
    onSend: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val palette = chatPalette
    val ready = draft.isNotBlank() && !pending

    val typing = remember { MutableInteractionSource() }
    val focused by typing.collectIsFocusedAsState()

    val pressing = remember { MutableInteractionSource() }
    val pressed by pressing.collectIsPressedAsState()

    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surface)
            .drawTopHairline(MaterialTheme.colorScheme.outlineVariant)
            .padding(start = 12.dp, end = 12.dp, top = 10.dp, bottom = 14.dp),
        // The field grows upwards, so the button stays level with its last line.
        verticalAlignment = Alignment.Bottom,
    ) {
        Box(
            modifier = Modifier
                .weight(1f)
                // Padded whether or not it shows, so taking focus moves nothing.
                .border(
                    width = if (focused) RING_WIDTH else 0.dp,
                    color = if (focused) MaterialTheme.colorScheme.primary else Color.Transparent,
                    shape = RING,
                )
                .padding(RING_GAP)
                .background(MaterialTheme.colorScheme.surfaceContainer, FIELD)
                .border(FIELD_BORDER, palette.fieldBorder, FIELD)
                .padding(horizontal = 14.dp, vertical = FIELD_PAD),
            contentAlignment = Alignment.CenterStart,
        ) {
            PlainField(
                value = draft,
                onValue = onDraft,
                placeholder = room?.let { "Message #$it" } ?: "Message",
                fontSize = 15.sp,
                enabled = !pending,
                maxLines = MAX_LINES,
                interactionSource = typing,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                keyboardActions = KeyboardActions(onSend = { if (ready) onSend() }),
                modifier = Modifier.fillMaxWidth(),
            )
        }

        Box(
            modifier = Modifier
                .padding(start = 10.dp, bottom = BUTTON_LIFT)
                .size(BUTTON)
                .background(
                    color = when {
                        !ready && !pending -> palette.sendDisabled
                        pressed -> palette.strongBlue
                        else -> MaterialTheme.colorScheme.primary
                    },
                    shape = PILL,
                )
                .clickable(
                    interactionSource = pressing,
                    indication = null,
                    enabled = ready,
                    onClick = onSend,
                ),
            contentAlignment = Alignment.Center,
        ) {
            if (pending) {
                Spinner(tint = MaterialTheme.colorScheme.onPrimary)
            } else {
                SendGlyph(tint = MaterialTheme.colorScheme.onPrimary)
            }
        }
    }
}
