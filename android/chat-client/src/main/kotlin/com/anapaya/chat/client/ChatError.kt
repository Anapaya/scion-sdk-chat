// Copyright 2026 Anapaya Systems

package com.anapaya.chat.client

/** Everything a call can fail with. */
public sealed class ChatError(
    message: String,
    cause: Throwable? = null,
) : Exception(message, cause) {
    public class Config(detail: String) : ChatError(detail)

    /** The request never produced a response. */
    public class Transport(
        public val kind: TransportFailure,
        detail: String,
        cause: Throwable? = null,
    ) : ChatError("$kind: $detail", cause)

    /** The server refused, and said why. `code` is a string, so a newer code still decodes. */
    public class Api(
        public val status: Int,
        public val code: String,
        public val serverMessage: String,
    ) : ChatError("$code ($status): $serverMessage")

    public class Protocol(detail: String) : ChatError(detail)

    public class NotLoggedIn : ChatError("not logged in")

    public class SessionExpired : ChatError("the session has expired")
}

/** Why a request did not complete, as the transport saw it. */
public enum class TransportFailure {
    Connectivity,

    Resolution,

    Connect,

    Tls,

    StreamReset,

    Protocol,

    BodyTooLarge,

    Timeout,

    Closed,
}
