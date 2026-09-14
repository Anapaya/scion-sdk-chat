// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.chat

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

private val PILL = RoundedCornerShape(percent = 50)

/** The height a field with one line in it stands at, and the button beside it. */
private val SEATED = 44.dp

/** Half [SEATED], so the field reads as a pill at rest and keeps those corners as it grows. */
private val FIELD = RoundedCornerShape(22.dp)

/** How far the field grows before it scrolls instead. */
private const val MAX_LINES = 5

/**
 * The line being typed, and the way to post it.
 *
 * The send button is the only thing on the screen that says a call is out, which is why the spinner
 * lives in it rather than in a bar of its own.
 */
@Composable
internal fun Composer(
    draft: String,
    room: String?,
    pending: Boolean,
    onDraft: (String) -> Unit,
    onSend: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val typed = draft.isNotBlank()
    val ready = typed && !pending

    Row(
        modifier = modifier
            .fillMaxWidth()
            .drawTopHairline(MaterialTheme.colorScheme.outlineVariant)
            .padding(start = 8.dp, end = 8.dp, top = 8.dp, bottom = 12.dp),
        // The field grows upwards, so the button stays level with its last line.
        verticalAlignment = Alignment.Bottom,
    ) {
        Box(
            modifier = Modifier
                .weight(1f)
                .heightIn(min = SEATED)
                .background(MaterialTheme.colorScheme.surfaceContainer, FIELD)
                .padding(horizontal = 16.dp, vertical = 12.dp),
            contentAlignment = Alignment.CenterStart,
        ) {
            PlainField(
                value = draft,
                onValue = onDraft,
                placeholder = room?.let { "Message #$it" } ?: "Message",
                fontSize = 14.5.sp,
                enabled = !pending,
                maxLines = MAX_LINES,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                keyboardActions = KeyboardActions(onSend = { if (ready) onSend() }),
                modifier = Modifier.fillMaxWidth(),
            )
        }

        // Nothing to press until there is something to post, so the field holds the width until
        // then.
        AnimatedVisibility(
            visible = typed || pending,
            enter = fadeIn() + scaleIn(initialScale = 0.7f),
            exit = fadeOut() + scaleOut(targetScale = 0.7f),
        ) {
            Box(
                modifier = Modifier
                    .padding(start = 8.dp)
                    .size(SEATED)
                    .background(MaterialTheme.colorScheme.primary, PILL)
                    .clickable(enabled = ready, onClick = onSend),
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
}
