// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.anapaya.chat.app.UiState

/** Where the network is. Pre-filled for the emulator, editable so a real device works too. */
@Composable
public fun ConnectScreen(
    state: UiState,
    onControlUrl: (String) -> Unit,
    onConnect: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text("Connect", style = MaterialTheme.typography.headlineMedium)
        Text(
            "The address chat-dev serves its description at. Everything after this goes over SCION.",
            style = MaterialTheme.typography.bodyMedium,
        )

        OutlinedTextField(
            value = state.controlUrl,
            onValueChange = onControlUrl,
            label = { Text("Control URL") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )

        Button(onClick = onConnect, enabled = !state.pending, modifier = Modifier.fillMaxWidth()) {
            Text("Connect")
        }

        Progress(state.pending)
        Failure(state.actionError)
    }
}

/** Registering and logging in are separate, as the API keeps them. */
@Composable
public fun SignInScreen(
    state: UiState,
    onRegister: (String, String) -> Unit,
    onLogIn: (String, String) -> Unit,
) {
    var username by rememberSaveable { mutableStateOf("") }
    var password by rememberSaveable { mutableStateOf("") }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text("Sign in", style = MaterialTheme.typography.headlineMedium)
        state.target?.let {
            Text("over SCION, to $it", style = MaterialTheme.typography.bodySmall)
        }

        OutlinedTextField(
            value = username,
            onValueChange = { username = it },
            label = { Text("Username") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = password,
            onValueChange = { password = it },
            label = { Text("Password") },
            singleLine = true,
            visualTransformation = PasswordVisualTransformation(),
            modifier = Modifier.fillMaxWidth(),
        )

        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            OutlinedButton(
                onClick = { onRegister(username, password) },
                enabled = !state.pending,
                modifier = Modifier.weight(1f),
            ) { Text("Register") }
            Button(
                onClick = { onLogIn(username, password) },
                enabled = !state.pending,
                modifier = Modifier.weight(1f),
            ) { Text("Log in") }
        }

        Progress(state.pending)
        Failure(state.actionError)
    }
}
