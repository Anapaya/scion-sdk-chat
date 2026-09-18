// Copyright 2026 Anapaya Systems

package com.anapaya.chat.client

/** One request, as the chat API asks for it. */
public data class ChatRequest(
    val method: String,
    /** Relative to the server's `/api/v1`, with a leading slash. */
    val path: String,
    val json: String? = null,
    /** The session token, on the calls that need one. */
    val bearer: String? = null,
)

/** What came back. The body is text: every reply this API sends is JSON. */
public data class ChatReply(
    val status: Int,
    val body: String,
)

/** Puts a request on the wire and brings the reply back. An interface, so a test can fake it. */
public interface Transport {
    public suspend fun send(request: ChatRequest): ChatReply

    /** Releases the connections. The client cannot be used afterwards. */
    public fun close()
}
