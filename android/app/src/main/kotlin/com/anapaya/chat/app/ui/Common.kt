// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui

import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/** A call the user asked for is out. */
@Composable
internal fun Progress(pending: Boolean) {
    if (pending) LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
}

/** Why something failed, in the shape both clients use. */
@Composable
internal fun Failure(message: String?, modifier: Modifier = Modifier) {
    message ?: return

    Text(
        text = "⚠ $message",
        color = MaterialTheme.colorScheme.error,
        style = MaterialTheme.typography.bodySmall,
        modifier = modifier.padding(vertical = 8.dp),
    )
}
