// Copyright 2026 Anapaya Systems

package com.anapaya.chat.client

/** Everything a call can fail with, mirroring `chat-client-core`'s `ChatError`. */
public sealed class ChatError(
    message: String,
    cause: Throwable? = null,
) : Exception(message, cause) {
    /** The configuration cannot be used: a bad address, a missing certificate. */
    public class Config(detail: String) : ChatError(detail)

    /** The request did not complete. */
    public class Transport(
        public val kind: TransportFailure,
        detail: String,
        cause: Throwable? = null,
    ) : ChatError("$kind: $detail", cause)

    /**
     * The server refused the request and said why.
     *
     * `code` is a string rather than an enum on purpose, the same reason `ErrorCode::Other` exists
     * on the server: a code added after this build shipped has to reach the user as the server's
     * own message, not as a decoding failure.
     */
    public class Api(
        public val status: Int,
        public val code: String,
        public val serverMessage: String,
    ) : ChatError("$code ($status): $serverMessage")

    /** A reply arrived that could not be decoded. */
    public class Protocol(detail: String) : ChatError(detail)

    /** The call needs a token and nobody has logged in. */
    public class NotLoggedIn : ChatError("not logged in")

    /** The token was refused and has been forgotten. Only logging in again fixes it. */
    public class SessionExpired : ChatError("the session has expired")
}

/**
 * Why a request did not complete, as the transport saw it.
 *
 * One value per arm of the SDK's `ScionHttp3Exception`, so nothing above the transport has to know
 * which transport produced it. See `ScionTransport.failure`.
 */
public enum class TransportFailure {
    /** The device has no usable network. */
    Connectivity,

    /** The server's name could not be resolved. */
    Resolution,

    /** No connection could be established. */
    Connect,

    /** The connection was refused on trust grounds. */
    Tls,

    /** The connection dropped mid-request. */
    StreamReset,

    /** The exchange did not follow HTTP. */
    Protocol,

    /** The reply was larger than this client accepts. */
    BodyTooLarge,

    /** The request took too long. */
    Timeout,

    /** The client has been closed. */
    Closed,
}
