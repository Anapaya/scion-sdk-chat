// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app.ui.chat

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.union
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.PermanentDrawerSheet
import androidx.compose.material3.PermanentNavigationDrawer
import androidx.compose.material3.Text
import androidx.compose.material3.rememberDrawerState
import androidx.compose.material3.windowsizeclass.WindowWidthSizeClass
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import com.anapaya.chat.app.UiState
import com.anapaya.chat.client.model.Room
import kotlinx.coroutines.flow.filter
import kotlinx.coroutines.launch

/** Below this the room list is a drawer over the conversation; at or above it, it stays open. */
private val PINNED = WindowWidthSizeClass.Expanded

/** How near the newest message the reader has to be for the list to keep following it. */
private val STICK = 40.dp

/**
 * The rooms, the open room's messages, and the line being typed.
 *
 * Everything the two layouts share is remembered here, above the branch between them: a composable
 * remembers by its place in the tree, and the two drawers are different places, so anything held
 * inside one would be dropped when a fold opens.
 */
@Composable
public fun ChatScreen(
    state: UiState,
    width: WindowWidthSizeClass,
    onOpenRoom: (Room) -> Unit,
    onSend: (String) -> Unit,
    onCreateRoom: (String) -> Unit,
    onDraftRestored: () -> Unit,
) {
    var draft by rememberSaveable { mutableStateOf("") }
    var creating by rememberSaveable { mutableStateOf(false) }
    val listState = rememberLazyListState()
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()

    val pinned = width == PINNED
    val clocks = remember { Clocks() }
    // Reversed as a view rather than a copy: rows are built oldest-first, the only order a change of
    // day or of author can be found in, and drawn newest-first.
    val rows = remember(state.messages, state.username) {
        chatRows(state.messages, state.username, clocks).asReversed()
    }

    // A send that never left hands its text back, so the reader does not lose what they typed.
    LaunchedEffect(state.restoredDraft) {
        state.restoredDraft?.let {
            draft = it
            onDraftRestored()
        }
    }

    // Whether the newest message should stay in view. Only a scroll that has settled may change it.
    // Reading the position as rows arrive would not work: a new message takes index 0 and moves the
    // reader's anchor to index 1, which is indistinguishable from the reader having scrolled away.
    val stick = with(LocalDensity.current) { STICK.toPx() }
    var following by remember { mutableStateOf(true) }
    LaunchedEffect(listState) {
        snapshotFlow { listState.isScrollInProgress }
            .filter { !it }
            .collect {
                following = listState.firstVisibleItemIndex == 0 &&
                    listState.firstVisibleItemScrollOffset <= stick
            }
    }

    // Keyed on the room so a switch cannot race the arrival it starts with, and on the newest seq
    // rather than the list, which changes identity for reasons that are not new messages.
    LaunchedEffect(state.openRoomId) {
        following = true
        listState.scrollToItem(0)
        snapshotFlow { state.messages.lastOrNull()?.seq }
            .collect { if (following) listState.animateScrollToItem(0) }
    }

    val sheet: @Composable () -> Unit = {
        RoomList(
            rooms = state.rooms,
            openRoomId = state.openRoomId,
            unread = state.unread,
            pending = state.pending,
            onOpenRoom = {
                // Re-opening the room already on screen closes the drawer and asks for nothing.
                if (it.id != state.openRoomId) onOpenRoom(it)
                if (!pinned) scope.launch { drawerState.close() }
            },
            onCreate = {
                creating = true
                if (!pinned) scope.launch { drawerState.close() }
            },
        )
    }

    val conversation: @Composable () -> Unit = {
        Conversation(
            state = state,
            rows = rows,
            draft = draft,
            listState = listState,
            detached = !following,
            // Removed rather than disabled where the list is already on screen: a dead button
            // leaves a hole where the title should start.
            onMenu = if (pinned) null else ({ scope.launch { drawerState.open() } }),
            onJump = {
                following = true
                scope.launch { listState.animateScrollToItem(0) }
            },
            onDraft = { draft = it },
            onSend = {
                onSend(draft)
                draft = ""
            },
        )
    }

    if (pinned) {
        PermanentNavigationDrawer(
            drawerContent = {
                PermanentDrawerSheet(
                    modifier = Modifier.width(DRAWER_WIDTH),
                    drawerContainerColor = MaterialTheme.colorScheme.surfaceContainerLowest,
                ) { sheet() }
            },
            content = { conversation() },
        )
    } else {
        BackHandler(enabled = drawerState.isOpen) { scope.launch { drawerState.close() } }
        ModalNavigationDrawer(
            drawerState = drawerState,
            scrimColor = MaterialTheme.colorScheme.scrim,
            drawerContent = {
                ModalDrawerSheet(
                    modifier = Modifier.width(DRAWER_WIDTH),
                    drawerContainerColor = MaterialTheme.colorScheme.surfaceContainerLowest,
                    drawerShape = RoundedCornerShape(topEnd = 12.dp, bottomEnd = 12.dp),
                ) { sheet() }
            },
            content = { conversation() },
        )
    }

    if (creating) {
        CreateRoomDialog(
            pending = state.pending,
            onDismiss = { creating = false },
            onCreate = {
                creating = false
                onCreateRoom(it)
            },
        )
    }
}

/**
 * The open room: its name, its messages, and the composer.
 *
 * Inside the drawer rather than around it, so a modal drawer's scrim covers the title bar and a
 * pinned list runs the full height beside it.
 */
@Composable
private fun Conversation(
    state: UiState,
    rows: List<ChatRow>,
    draft: String,
    listState: LazyListState,
    detached: Boolean,
    onMenu: (() -> Unit)?,
    onJump: () -> Unit,
    onDraft: (String) -> Unit,
    onSend: () -> Unit,
) {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.surface),
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            TopBar(
                room = state.openRoom?.name,
                username = state.username,
                unreadElsewhere = state.unread.isNotEmpty(),
                onMenu = onMenu,
            )

            MessageList(
                rows = rows,
                listState = listState,
                modifier = Modifier.weight(1f),
            )

            Column(
                // The keyboard's inset is measured from the bottom of the window and already covers
                // the navigation bar, so the two are unioned rather than added.
                modifier = Modifier.windowInsetsPadding(
                    WindowInsets.ime.union(WindowInsets.navigationBars),
                ),
            ) {
                // A read that failed and something the reader asked for that failed are different
                // news: one clears itself on the next poll, the other has to outlive it.
                state.feedError?.let { ErrorBanner(it) }
                state.actionError?.let { ErrorBanner(it) }

                Composer(
                    draft = draft,
                    room = state.openRoom?.name,
                    pending = state.pending,
                    onDraft = onDraft,
                    onSend = onSend,
                )
            }
        }

        if (detached) {
            JumpToNewest(
                onClick = onJump,
                modifier = Modifier.align(Alignment.BottomCenter).padding(bottom = 104.dp),
            )
        }
    }
}

@Composable
private fun TopBar(
    room: String?,
    username: String?,
    unreadElsewhere: Boolean,
    onMenu: (() -> Unit)?,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .statusBarsPadding()
            .height(64.dp)
            .drawBottomHairline(MaterialTheme.colorScheme.outlineVariant)
            .padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        onMenu?.let { open ->
            Box(
                modifier = Modifier
                    .size(48.dp)
                    .clickable(
                        interactionSource = remember { MutableInteractionSource() },
                        indication = null,
                        onClick = open,
                    ),
                contentAlignment = Alignment.Center,
            ) {
                MenuGlyph(tint = MaterialTheme.colorScheme.onSurface)

                // Says a room the reader is not in has something new, which is the only reason to
                // open a list they otherwise have no cause to look at.
                if (unreadElsewhere) {
                    Box(
                        modifier = Modifier
                            .align(Alignment.TopEnd)
                            .padding(top = 9.dp, end = 10.dp)
                            // 9dp of blue, and a 2dp ring of the page around it so the dot reads
                            // against whichever bar it lands on.
                            .size(13.dp)
                            .background(MaterialTheme.colorScheme.surface, RoundedCornerShape(percent = 50))
                            .padding(2.dp)
                            .background(MaterialTheme.colorScheme.primary, RoundedCornerShape(percent = 50)),
                    )
                }
            }
        }

        Column(modifier = Modifier.weight(1f).padding(horizontal = 8.dp)) {
            Text(
                text = room?.let { "#$it" } ?: "no rooms",
                fontSize = 20.sp,
                fontWeight = FontWeight.SemiBold,
                letterSpacing = (-0.01).em,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            username?.let {
                Text(
                    text = "signed in as $it",
                    fontSize = 11.5.sp,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 1.dp),
                )
            }
        }
    }
}
