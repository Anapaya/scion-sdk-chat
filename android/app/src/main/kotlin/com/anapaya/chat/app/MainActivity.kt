// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.material3.Surface
import androidx.compose.material3.windowsizeclass.ExperimentalMaterial3WindowSizeClassApi
import androidx.compose.material3.windowsizeclass.WindowWidthSizeClass
import androidx.compose.material3.windowsizeclass.calculateWindowSizeClass
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.repeatOnLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.anapaya.chat.app.ui.ConnectScreen
import com.anapaya.chat.app.ui.SignInScreen
import com.anapaya.chat.app.ui.chat.ChatScreen
import com.anapaya.chat.app.ui.theme.ChatTheme

public class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        // Before the content, so the window is already laid out behind the system bars when the
        // first frame draws. Every screen pads itself back off them.
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)

        setContent {
            // Measured here rather than in `ui/`, which keeps the opt-in and the activity out of
            // the screens and lets a preview name a width outright.
            @OptIn(ExperimentalMaterial3WindowSizeClassApi::class)
            val width = calculateWindowSizeClass(this).widthSizeClass

            ChatTheme {
                Surface { Chat(width) }
            }
        }
    }
}

@Composable
private fun Chat(width: WindowWidthSizeClass, model: ChatViewModel = viewModel()) {
    val state by model.state.collectAsStateWithLifecycle()

    // The poll lifecycle: polling runs only while this screen is at least STARTED, so a
    // backgrounded app stops asking for messages nobody is reading, and resumes on return without
    // rebuilding the client.
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    LaunchedEffect(state.screen) {
        if (state.screen is Screen.Chat) {
            lifecycle.repeatOnLifecycle(Lifecycle.State.STARTED) {
                model.startPolling()
                try {
                    kotlinx.coroutines.awaitCancellation()
                } finally {
                    model.stopPolling()
                }
            }
        }
    }

    when (state.screen) {
        Screen.Connect -> ConnectScreen(state, model::controlUrlChanged, model::connect)
        Screen.SignIn -> SignInScreen(state, model::register, model::logIn)
        Screen.Chat -> ChatScreen(
            state = state,
            width = width,
            onOpenRoom = model::openRoom,
            onSend = model::send,
            onCreateRoom = model::createRoom,
            onDraftRestored = model::draftRestored,
        )
    }
}
