// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.chat

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

private val PILL = RoundedCornerShape(percent = 50)

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
    val ready = draft.isNotBlank() && !pending

    Row(
        modifier = modifier
            .fillMaxWidth()
            .drawTopHairline(MaterialTheme.colorScheme.outlineVariant)
            .padding(start = 8.dp, end = 8.dp, top = 8.dp, bottom = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Box(
            modifier = Modifier
                .weight(1f)
                .height(44.dp)
                .background(MaterialTheme.colorScheme.surfaceContainer, PILL)
                .padding(horizontal = 16.dp),
            contentAlignment = Alignment.CenterStart,
        ) {
            PlainField(
                value = draft,
                onValue = onDraft,
                placeholder = room?.let { "Message #$it" } ?: "Message",
                fontSize = 14.5.sp,
                enabled = !pending,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                keyboardActions = KeyboardActions(onSend = { if (ready) onSend() }),
                modifier = Modifier.fillMaxWidth(),
            )
        }

        Box(
            modifier = Modifier
                .padding(start = 8.dp)
                .size(44.dp)
                .background(MaterialTheme.colorScheme.primary, PILL)
                .clickable(enabled = ready, onClick = onSend)
                .alpha(if (ready || pending) 1f else 0.38f),
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
