// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.anapaya.chat.client.ChatClient
import com.anapaya.chat.client.ChatError
import com.anapaya.chat.client.DevNetwork
import com.anapaya.chat.client.ScionTransport
import com.anapaya.chat.client.model.Message
import com.anapaya.chat.client.model.Room
import com.anapaya.chat.client.roomList
import com.anapaya.chat.client.roomMessages
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.retryWhen
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** How long a feed waits before reading again, after a read that failed. */
private const val RETRY_MILLIS = 2_000L

/** Which screen is showing. The flow is one way, except that an ended session goes back to signing in. */
public sealed interface Screen {
    public data object Connect : Screen

    public data object SignIn : Screen

    public data object Chat : Screen
}

/**
 * Everything the screens draw.
 *
 * Every field has value semantics, which is what lets [MutableStateFlow] drop the room list the
 * feed re-reads every couple of seconds: an unchanged listing produces an equal state and costs no
 * recomposition. A `Throwable`, a `Job` or a lambda in here would compare by identity and turn that
 * poll into a recomposition every two seconds.
 */
public data class UiState(
    val screen: Screen = Screen.Connect,
    val controlUrl: String = DevNetwork.DEFAULT_CONTROL_URL,
    val rooms: List<Room> = emptyList(),
    /**
     * Which room is open, by id rather than by value: the listing is re-read every couple of
     * seconds, and a held [Room] carries a `latestSeq` that is stale within one poll.
     */
    val openRoomId: Long? = null,
    val messages: List<Message> = emptyList(),
    /** The rooms holding something the reader has not seen. See [Unread] for why it is not a count. */
    val unread: Set<Long> = emptySet(),
    val username: String? = null,
    /** Whether a call the user asked for is still out. One at a time. */
    val pending: Boolean = false,
    /** Why the last read failed. Cleared by the next read that works. */
    val feedError: String? = null,
    /**
     * Why the last thing the user asked for failed, held until they ask for something else.
     *
     * Separate from [feedError] because a read works every second or so, and a read working says
     * nothing about a send that did not: one clearing the other would take the reason off the
     * screen before it was read.
     */
    val actionError: String? = null,
    /** Text a refused send is handing back to the composer. Taken once, then acknowledged. */
    val restoredDraft: String? = null,
    /** Where the server is, once it is known. Shown so the SCION address is visible. */
    val target: String? = null,
) {
    /** The open room as the last listing described it, so it cannot go stale. */
    val openRoom: Room? get() = rooms.firstOrNull { it.id == openRoomId }
}

/**
 * Adds a batch, keeping one row per `seq`.
 *
 * Polling resumes from the newest message it has seen, but a resumed feed starts again from the
 * newest page, so a batch can repeat what is already held. `seq` is server-wide and strictly
 * increasing, which makes it the identity to merge on.
 *
 * The held messages come first on purpose: `distinctBy` keeps the first of each `seq`, so a
 * message already on screen keeps the instance already drawn and its row is skipped rather than
 * recomposed.
 */
private fun UiState.merge(batch: List<Message>): UiState =
    copy(messages = (messages + batch).distinctBy { it.seq }.sortedBy { it.seq })

/**
 * Every call to the chat client, and the state the screens draw.
 *
 * The screens draw and report what was tapped; nothing in `ui/` talks to a server, so there is one
 * place to look for how the SDK is used.
 */
public class ChatViewModel(application: Application) : AndroidViewModel(application) {
    private val _state = MutableStateFlow(UiState())
    public val state: StateFlow<UiState> = _state.asStateFlow()

    private var client: ChatClient? = null

    private val unread = Unread()

    /** The two feeds, stopped whenever the app leaves the foreground. */
    private var roomsJob: Job? = null

    /** Restarted on its own when a room is opened, which the room list has no reason to be. */
    private var messagesJob: Job? = null

    public fun controlUrlChanged(url: String) {
        _state.update { it.copy(controlUrl = url) }
    }

    /** Reads the network's description, builds a client, and proves the server is there. */
    public fun connect() {
        ask {
            val network = DevNetwork.discover(_state.value.controlUrl)
            val built = ChatClient(ScionTransport(getApplication(), network))
            // Building only parses configuration; nothing is dialled until a call is made. The
            // health check is what turns a wrong address into an error on this screen.
            built.health()

            client = built
            _state.update { it.copy(screen = Screen.SignIn, target = network.target) }
        }
    }

    public fun register(username: String, password: String) {
        ask { requireClient().register(username, password) }
    }

    public fun logIn(username: String, password: String) {
        ask {
            val client = requireClient()
            client.logIn(username, password)

            val rooms = client.rooms()
            val open = rooms.firstOrNull()
            // Seeded before anything is drawn, so the first frame is quiet.
            unread.seed(rooms)

            _state.update {
                it.copy(
                    screen = Screen.Chat,
                    username = username,
                    rooms = rooms,
                    openRoomId = open?.id,
                    messages = emptyList(),
                    unread = unread.of(rooms, open?.id),
                )
            }
        }
    }

    public fun openRoom(room: Room) {
        // Deliberately does not advance the read cursor: only a delivered batch does. Marking a
        // room read on opening it would silence a room whose messages never arrived.
        _state.update {
            it.copy(
                openRoomId = room.id,
                messages = emptyList(),
                unread = unread.of(it.rooms, room.id),
            )
        }
        watchMessages()
    }

    /**
     * Posts what was typed.
     *
     * Every line is a message. Rooms are made from the drawer, so there is no syntax to learn and
     * nothing a message can be mistaken for.
     */
    public fun send(body: String) {
        val typed = body.trim()
        if (typed.isEmpty()) return

        val room = _state.value.openRoomId ?: return
        // Handed back rather than dropped: the composer has already been cleared, and a refusal
        // the reader cannot see would look like a message that was sent.
        if (!ask({ restore(typed) }) { requireClient().send(room, typed) }) restore(typed)
    }

    /**
     * Creates a room, refusing a name this client will not accept before any call is made.
     *
     * Nothing is added here: the room list feed picks the new room up within a couple of seconds,
     * which is the same path a room somebody else created arrives by.
     *
     * Reached only from the drawer. The terminal client also takes `/room <name>` typed into its
     * composer; this one does not, so a line typed here is always a message.
     */
    public fun createRoom(name: String) {
        val problem = roomNameProblem(name)
        if (problem != null) {
            _state.update { it.copy(actionError = problem) }
            return
        }

        ask { requireClient().createRoom(name) }
    }

    /** Acknowledges [UiState.restoredDraft], once the composer holds it. */
    public fun draftRestored() {
        _state.update { it.copy(restoredDraft = null) }
    }

    /**
     * Starts both feeds, and stops them again when the app is no longer in the foreground.
     *
     * Called from the screen's lifecycle rather than on connecting, so a backgrounded app is not
     * asking a server for messages nobody is reading.
     */
    public fun startPolling() {
        if (client == null) return
        if (roomsJob?.isActive != true) watchRooms()
        if (messagesJob?.isActive != true) watchMessages()
    }

    public fun stopPolling() {
        roomsJob?.cancel()
        roomsJob = null
        messagesJob?.cancel()
        messagesJob = null
    }

    /**
     * Watches the room list.
     *
     * Runs whether or not a room is open, so a room created elsewhere can be discovered by a client
     * that has none.
     */
    private fun watchRooms() {
        roomsJob?.cancel()
        val client = client ?: return

        roomsJob = viewModelScope.launch {
            client.roomList()
                .retryWhen { cause, _ -> feedFailed(cause).also { if (it) delay(RETRY_MILLIS) } }
                .collect { rooms ->
                    _state.update {
                        it.copy(
                            rooms = rooms,
                            feedError = null,
                            unread = unread.of(rooms, it.openRoomId),
                        )
                    }
                }
        }
    }

    private fun watchMessages() {
        messagesJob?.cancel()
        val client = client ?: return
        val room = _state.value.openRoomId ?: return

        messagesJob = viewModelScope.launch {
            client.roomMessages(room)
                .retryWhen { cause, _ -> feedFailed(cause).also { if (it) delay(RETRY_MILLIS) } }
                .collect { batch -> applyBatch(room, batch) }
        }
    }

    /**
     * Takes a batch, and marks the room it belongs to read up to it.
     *
     * The room is checked rather than assumed: cancelling a feed is cooperative, so a batch already
     * in flight when the reader moved on would otherwise land in the room they moved to.
     */
    private fun applyBatch(room: Long, batch: List<Message>) {
        _state.update { current ->
            if (room != current.openRoomId) return@update current

            // `maxOfOrNull` rather than `maxOf`: the feed only emits a batch it found something in,
            // and a crash is a steep price for relying on that from here.
            batch.maxOfOrNull { it.seq }?.let { unread.advance(room, it) }
            current.merge(batch).copy(
                feedError = null,
                unread = unread.of(current.rooms, room),
            )
        }
    }

    /**
     * Runs a call the user asked for, refusing a second while one is out.
     *
     * Answers whether the call was taken, so a caller holding something the reader typed can put it
     * back rather than lose it. `refused` runs for a call that was taken and then failed.
     */
    private fun ask(refused: () -> Unit = {}, work: suspend () -> Unit): Boolean {
        if (_state.value.pending) return false
        _state.update { it.copy(pending = true, actionError = null) }

        viewModelScope.launch {
            try {
                work()
            } catch (error: Exception) {
                if (!signedOut(error)) {
                    _state.update { it.copy(actionError = error.message ?: error.toString()) }
                    refused()
                }
            } finally {
                _state.update { it.copy(pending = false) }
            }
        }
        return true
    }

    /** Hands text back to the composer, for a send that never left. */
    private fun restore(body: String) {
        _state.update { it.copy(restoredDraft = body) }
    }

    /**
     * Records a read that failed, and answers whether the feed should keep trying.
     *
     * Only an ended session stops it. Anything else is worth another read: the server may be
     * restarting, and a feed that gave up would never reach the read that clears the row.
     */
    private fun feedFailed(error: Throwable): Boolean {
        if (signedOut(error)) return false
        _state.update { it.copy(feedError = error.message ?: error.toString()) }
        return true
    }

    /** Sends the reader back to signing in, if this is the failure that means they must. */
    private fun signedOut(error: Throwable): Boolean {
        if (error !is ChatError.SessionExpired) return false

        stopPolling()
        _state.update {
            it.copy(
                screen = Screen.SignIn,
                actionError = error.message,
                messages = emptyList(),
                unread = emptySet(),
            )
        }
        return true
    }

    private fun requireClient(): ChatClient = client ?: throw ChatError.NotLoggedIn()

    override fun onCleared() {
        stopPolling()
        client?.close()
    }
}
