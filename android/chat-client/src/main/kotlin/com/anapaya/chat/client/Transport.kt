// Copyright 2026 Anapaya Systems

package com.anapaya.chat.client

/** One exchange with the server, in the terms [ChatClient] thinks in. */
public data class ChatRequest(
    val method: String,
    /** Relative to the server's `/api/v1`, with a leading slash. */
    val path: String,
    val json: String? = null,
    /** The session token, on the calls that need one. */
    val bearer: String? = null,
)

/** What came back. The body is text because every reply this API sends is JSON. */
public data class ChatReply(
    val status: Int,
    val body: String,
)

/**
 * How [ChatClient] reaches a server.
 *
 * An interface with one implementation, [ScionTransport]. It is here so the boundary is visible:
 * everything above it is ordinary HTTP and JSON, and nothing above it mentions SCION.
 */
public interface Transport {
    public suspend fun send(request: ChatRequest): ChatReply

    /** Releases the connections. The client cannot be used afterwards. */
    public fun close()
}
