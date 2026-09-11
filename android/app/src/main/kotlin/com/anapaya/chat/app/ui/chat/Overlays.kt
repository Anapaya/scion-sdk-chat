// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.chat

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.anapaya.chat.app.roomNameProblem
import com.anapaya.chat.app.ui.theme.chatPalette

/**
 * Names a new room.
 *
 * The only way to make one here. The name is checked as it is typed, so a name the server would
 * refuse never costs a round trip.
 */
@Composable
internal fun CreateRoomDialog(pending: Boolean, onDismiss: () -> Unit, onCreate: (String) -> Unit) {
    var name by rememberSaveable { mutableStateOf("") }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { focus.requestFocus() }

    val problem = roomNameProblem(name)
    val submit = { if (problem == null && !pending) onCreate(name) }

    Dialog(onDismissRequest = onDismiss) {
        Column(
            modifier = Modifier
                .widthIn(max = 288.dp)
                .background(MaterialTheme.colorScheme.surfaceContainerLowest, RoundedCornerShape(12.dp))
                .padding(20.dp),
        ) {
            Text("New room", fontSize = 17.sp, fontWeight = FontWeight.SemiBold)
            Text(
                text = "Creates the room on the server and joins it.",
                fontSize = 12.sp,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 4.dp),
            )

            NameField(
                name = name,
                onName = { name = it },
                onSubmit = submit,
                modifier = Modifier.padding(top = 16.dp).focusRequester(focus),
            )

            // Nothing is wrong with a field nobody has typed in yet.
            if (name.isNotEmpty() && problem != null) {
                Text(
                    text = problem,
                    fontSize = 11.5.sp,
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.padding(top = 6.dp),
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth().padding(top = 16.dp),
                horizontalArrangement = Arrangement.End,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                TextButton(onClick = onDismiss, modifier = Modifier.height(40.dp)) {
                    Text("Cancel", color = MaterialTheme.colorScheme.onSurfaceVariant, fontSize = 13.5.sp)
                }
                Button(
                    onClick = submit,
                    enabled = problem == null && !pending,
                    shape = RoundedCornerShape(percent = 50),
                    colors = ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.primary),
                    modifier = Modifier.padding(start = 8.dp).height(40.dp),
                ) {
                    Text(if (pending) "Creating…" else "Create", fontSize = 13.5.sp, fontWeight = FontWeight.SemiBold)
                }
            }
        }
    }
}

@Composable
private fun NameField(
    name: String,
    onName: (String) -> Unit,
    onSubmit: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Row(
        modifier = modifier
            .fillMaxWidth()
            .height(46.dp)
            .background(MaterialTheme.colorScheme.surfaceContainer, RoundedCornerShape(8.dp))
            .border(1.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(8.dp))
            .padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = "#",
            fontSize = 15.sp,
            fontFamily = FontFamily.Monospace,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        PlainField(
            value = name,
            onValue = onName,
            placeholder = "room-name",
            fontSize = 15.sp,
            keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
            keyboardActions = KeyboardActions(onDone = { onSubmit() }),
            modifier = Modifier.weight(1f).padding(start = 6.dp),
        )
    }
}

/** Why something failed, held until the next call works. Not dismissible: there is nothing to do. */
@Composable
internal fun ErrorBanner(message: String, modifier: Modifier = Modifier) {
    val palette = chatPalette

    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(palette.riskTint)
            .drawTopHairline(palette.riskBorder)
            .padding(horizontal = 16.dp, vertical = 10.dp),
    ) {
        Box(
            modifier = Modifier
                .padding(top = 5.dp, end = 10.dp)
                .size(8.dp)
                .background(palette.risk, RoundedCornerShape(percent = 50)),
        )
        // The message takes the page's ink, not the orange: orange on its own tint does not carry
        // at this size.
        Text(
            text = message,
            fontSize = 12.5.sp,
            lineHeight = 12.5.sp * 1.45f,
            color = MaterialTheme.colorScheme.onSurface,
        )
    }
}
