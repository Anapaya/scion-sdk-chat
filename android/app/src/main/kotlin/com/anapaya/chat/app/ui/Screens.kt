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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import com.anapaya.chat.app.CredentialChoice
import com.anapaya.chat.app.ManualForm
import com.anapaya.chat.app.TrustChoice
import com.anapaya.chat.app.UiState

/** Where a network that describes itself is asked for that description. */
@Composable
public fun ConnectScreen(
    state: UiState,
    onControlUrl: (String) -> Unit,
    onConnect: () -> Unit,
    onManual: () -> Unit,
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
            "The address a development network serves its description at. Everything after this " +
                "goes over SCION.",
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
        TextButton(
            onClick = onManual,
            enabled = !state.pending,
            modifier = Modifier.fillMaxWidth(),
        ) { Text("Manual SCION configuration") }

        Progress(state.pending)
        Failure(state.actionError)
    }
}

/** The same configuration, typed out, for a network that describes nothing. */
@Composable
public fun ManualScreen(
    state: UiState,
    onForm: (ManualForm) -> Unit,
    onConnect: () -> Unit,
    onBack: () -> Unit,
) {
    val form = state.manual

    Column(
        modifier = Modifier
            .fillMaxSize()
            .windowInsetsPadding(WindowInsets.safeDrawing)
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text("SCION configuration", style = MaterialTheme.typography.headlineMedium)

        Field("Endhost API", form.endhostApiUrl) { onForm(form.copy(endhostApiUrl = it)) }
        Field("Server URL", form.baseUrl) { onForm(form.copy(baseUrl = it)) }
        Text("Credential", style = MaterialTheme.typography.labelMedium)
        SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
            CredentialChoice.entries.forEachIndexed { index, choice ->
                SegmentedButton(
                    selected = form.credential == choice,
                    onClick = { onForm(form.copy(credential = choice)) },
                    shape = SegmentedButtonDefaults.itemShape(index, CredentialChoice.entries.size),
                ) { Text(choice.label) }
            }
        }
        when (form.credential) {
            CredentialChoice.ApiKey ->
                Field("Auth API key", form.authApiKey) { onForm(form.copy(authApiKey = it)) }
            CredentialChoice.Token ->
                Field("SNAP token", form.snapToken) { onForm(form.copy(snapToken = it)) }
        }

        Field("Target - the server's SCION address", form.target) { onForm(form.copy(target = it)) }

        Text("Trust", style = MaterialTheme.typography.labelMedium)
        SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
            TrustChoice.entries.forEachIndexed { index, choice ->
                SegmentedButton(
                    selected = form.trust == choice,
                    onClick = { onForm(form.copy(trust = choice)) },
                    shape = SegmentedButtonDefaults.itemShape(index, TrustChoice.entries.size),
                ) { Text(choice.label) }
            }
        }
        if (form.trust == TrustChoice.Pinned) {
            Field("Certificate (PEM)", form.certPem, lines = 4) { onForm(form.copy(certPem = it)) }
        }

        Button(onClick = onConnect, enabled = !state.pending, modifier = Modifier.fillMaxWidth()) {
            Text("Connect")
        }
        TextButton(
            onClick = onBack,
            enabled = !state.pending,
            modifier = Modifier.fillMaxWidth(),
        ) { Text("Read it from a development network") }

        Progress(state.pending)
        Failure(state.actionError)
    }
}

@Composable
private fun Field(
    label: String,
    value: String,
    lines: Int = 1,
    onValue: (String) -> Unit,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValue,
        label = { Text(label) },
        singleLine = lines == 1,
        maxLines = lines,
        modifier = Modifier.fillMaxWidth(),
    )
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
