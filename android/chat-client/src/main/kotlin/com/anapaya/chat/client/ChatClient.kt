// Copyright 2026 Anapaya Systems

package com.anapaya.chat.client

import com.anapaya.chat.client.model.CreateRoomRequest
import com.anapaya.chat.client.model.LoginRequest
import com.anapaya.chat.client.model.LoginResponse
import com.anapaya.chat.client.model.Message
import com.anapaya.chat.client.model.MessagesResponse
import com.anapaya.chat.client.model.PostMessageRequest
import com.anapaya.chat.client.model.RegisterRequest
import com.anapaya.chat.client.model.Room
import com.anapaya.chat.client.model.RoomsResponse
import com.anapaya.chat.client.model.ServerInfo
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

/** The chat API, over whatever [Transport] it is given. Holds the session token. */
public class ChatClient(private val transport: Transport) {
    private var token: String? = null

    /** Who is logged in. */
    public var username: String? = null
        private set

    public suspend fun health() {
        call("GET", "/healthz", authenticated = false)
    }

    public suspend fun serverInfo(): ServerInfo =
        decode(call("GET", "/server", authenticated = false))

    /** Creates the account. Does not log in: the server keeps the two apart. */
    public suspend fun register(username: String, password: String) {
        call(
            "POST",
            "/register",
            json = format.encodeToString(RegisterRequest(password = password, username = username)),
            authenticated = false,
        )
    }

    public suspend fun logIn(username: String, password: String) {
        val body = format.encodeToString(LoginRequest(password = password, username = username))
        val reply: LoginResponse = decode(call("POST", "/login", json = body, authenticated = false))

        token = reply.token
        this.username = username
    }

    public suspend fun rooms(): List<Room> = decode<RoomsResponse>(call("GET", "/rooms")).rooms

    public suspend fun createRoom(name: String): Room =
        decode(call("POST", "/rooms", json = format.encodeToString(CreateRoomRequest(name))))

    public suspend fun messagesNewest(room: Long, limit: Int = PAGE): List<Message> =
        decode<MessagesResponse>(call("GET", "/rooms/$room/messages?limit=$limit")).messages

    public suspend fun messagesAfter(room: Long, after: Long, limit: Int = PAGE): List<Message> =
        decode<MessagesResponse>(
            call("GET", "/rooms/$room/messages?after_seq=$after&limit=$limit"),
        ).messages

    public suspend fun send(room: Long, body: String) {
        call("POST", "/rooms/$room/messages", json = format.encodeToString(PostMessageRequest(body)))
    }

    public fun close(): Unit = transport.close()

    /** Sends, and turns anything that is not a success into a [ChatError]. */
    private suspend fun call(
        method: String,
        path: String,
        json: String? = null,
        authenticated: Boolean = true,
    ): String {
        val bearer = when {
            !authenticated -> null
            else -> token ?: throw ChatError.NotLoggedIn()
        }

        val reply = transport.send(ChatRequest(method, path, json, bearer))
        if (reply.status in 200..299) {
            return reply.body
        }

        val failure = refusal(reply.status, reply.body)
        // The token is gone, not refused, so retrying with it cannot work.
        if (failure is ChatError.SessionExpired) {
            token = null
            username = null
        }

        throw failure
    }

    private inline fun <reified T> decode(body: String): T =
        try {
            format.decodeFromString<T>(body)
        } catch (error: Exception) {
            throw ChatError.Protocol("a reply could not be decoded: ${error.message}")
        }

    private companion object {
        /** How many messages a page holds. */
        const val PAGE = 50

        val format = Json { ignoreUnknownKeys = true }

        fun refusal(status: Int, body: String): ChatError {
            val envelope = try {
                format.decodeFromString<ErrorEnvelope>(body)
            } catch (error: Exception) {
                return ChatError.Protocol("$status carried no error envelope: ${error.message}")
            }

            return when (envelope.error.code) {
                "expired_token", "unauthorized" -> ChatError.SessionExpired()
                else -> ChatError.Api(status, envelope.error.code, envelope.error.message)
            }
        }
    }
}

/** The body of every failing response. `code` is a `String`, so a code added later still decodes. */
@Serializable
private data class ErrorEnvelope(@SerialName("error") val error: ApiFailure)

@Serializable
private data class ApiFailure(
    @SerialName("code") val code: String,
    @SerialName("message") val message: String,
)
