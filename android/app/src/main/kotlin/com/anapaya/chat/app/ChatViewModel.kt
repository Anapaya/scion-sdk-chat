// Copyright 2026 Anapaya Systems

package com.anapaya.chat.app

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.anapaya.chat.client.ChatClient
import com.anapaya.chat.client.ChatError
import com.anapaya.chat.client.DevNetwork
import com.anapaya.chat.client.ScionConfig
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

private const val RETRY_MILLIS = 2_000L

/** Which screen is showing. */
public sealed interface Screen {
    public data object Connect : Screen

    /** Connect, with the configuration typed out. */
    public data object Manual : Screen

    public data object SignIn : Screen

    public data object Chat : Screen
}

/** A SCION configuration as it is typed. A blank field means the network answers for it. */
public data class ManualForm(
    val endhostApiUrl: String = "",
    val baseUrl: String = "",
    val snapToken: String = "",
    val target: String = "",
    val certPem: String = "",
) {
    public fun toScionConfig(): ScionConfig = ScionConfig(
        endhostApiUrl = endhostApiUrl.trim(),
        baseUrl = baseUrl.trim(),
        snapToken = snapToken.trim().ifBlank { null },
        target = target.trim().ifBlank { null },
        certPem = certPem.trim().ifBlank { null },
    )
}

/** Everything the screens draw. */
public data class UiState(
    val screen: Screen = Screen.Connect,
    val controlUrl: String = DevNetwork.DEFAULT_CONTROL_URL,
    val manual: ManualForm = ManualForm(),
    val rooms: List<Room> = emptyList(),
    val openRoomId: Long? = null,
    val messages: List<Message> = emptyList(),
    val unread: Set<Long> = emptySet(),
    val username: String? = null,
    val pending: Boolean = false,
    /** Why the last read failed. Cleared by the next read that works. */
    val feedError: String? = null,
    /** Why the last thing the reader asked for failed. Held until they ask for something else. */
    val actionError: String? = null,
    val restoredDraft: String? = null,
    val notice: String? = null,
    val target: String? = null,
) {
    val openRoom: Room? get() = rooms.firstOrNull { it.id == openRoomId }
}

/** Held messages first: `distinctBy` keeps the first of each `seq`, so a drawn row is not replaced. */
private fun UiState.merge(batch: List<Message>): UiState =
    copy(messages = (messages + batch).distinctBy { it.seq }.sortedBy { it.seq })

/** Every call to the chat client, and the state the screens draw. */
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

    public fun manualChanged(form: ManualForm) {
        _state.update { it.copy(manual = form) }
    }

    /** Moves between the two connect screens, dropping what the other one's attempt reported. */
    public fun showConnect(screen: Screen) {
        _state.update { it.copy(screen = screen, actionError = null) }
    }

    /** Reads the network's description, then connects with it. */
    public fun connect() {
        ask { open(DevNetwork.discover(_state.value.controlUrl).toScionConfig()) }
    }

    /** Connects with a configuration typed in full. */
    public fun connectManually() {
        ask {
            val config = _state.value.manual.toScionConfig()
            if (config.endhostApiUrl.isEmpty() || config.baseUrl.isEmpty()) {
                throw ChatError.Config("an endhost API and a server URL are both needed")
            }
            open(config)
        }
    }

    /** Builds a client, and proves the server is there. */
    private suspend fun open(config: ScionConfig) {
        val built = ChatClient(ScionTransport(getApplication(), config))
        // Building only parses configuration; nothing is dialled until a call is made. The health
        // check is what turns a wrong address into an error on this screen.
        built.health()

        client = built
        _state.update {
            it.copy(screen = Screen.SignIn, target = config.target ?: config.baseUrl)
        }
    }

    public fun register(username: String, password: String) {
        ask(refused = { why -> announce("Could not register $username: $why") }) {
            requireClient().register(username, password)
            announce("Registered $username. Log in to continue.")
        }
    }

    /** Clears the toast once it has been shown, so a recomposition does not repeat it. */
    public fun noticeShown() {
        _state.update { it.copy(notice = null) }
    }

    private fun announce(text: String) {
        _state.update { it.copy(notice = text) }
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

    /** Posts what was typed. */
    public fun send(body: String) {
        val typed = body.trim()
        if (typed.isEmpty()) return

        val room = _state.value.openRoomId ?: return
        // Handed back rather than dropped: the composer has already been cleared, and a refusal
        // the reader cannot see would look like a message that was sent.
        if (!ask({ restore(typed) }) { requireClient().send(room, typed) }) restore(typed)
    }

    /** Creates a room, refusing a name this client will not accept before any call is made. */
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

    /** Starts both feeds. Called from the screen's lifecycle, so a backgrounded app stops reading. */
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

    /** Watches the room list. */
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

    /** Takes a batch, and marks the room it belongs to read up to it. */
    private fun applyBatch(room: Long, batch: List<Message>) {
        _state.update { current ->
            if (room != current.openRoomId) return@update current

            batch.maxOfOrNull { it.seq }?.let { unread.advance(room, it) }
            current.merge(batch).copy(
                feedError = null,
                unread = unread.of(current.rooms, room),
            )
        }
    }

    /** Runs a call the reader asked for, refusing a second while one is out. */
    private fun ask(refused: (String) -> Unit = {}, work: suspend () -> Unit): Boolean {
        if (_state.value.pending) return false
        _state.update { it.copy(pending = true, actionError = null) }

        viewModelScope.launch {
            try {
                work()
            } catch (error: Exception) {
                if (!signedOut(error)) {
                    val why = error.message ?: error.toString()
                    _state.update { it.copy(actionError = why) }
                    refused(why)
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

    /** Records a read that failed, and answers whether the feed should keep trying. */
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
