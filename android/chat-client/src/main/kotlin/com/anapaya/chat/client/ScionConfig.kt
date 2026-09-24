// Copyright 2026 Anapaya Systems

package com.anapaya.chat.client

/** What the SDK needs to reach a server over SCION. */
public data class ScionConfig(
    /** The endhost API of the AS this client attaches to. */
    val endhostApiUrl: String,
    /** Where the server is. Its host is the name the certificate is issued for. */
    val baseUrl: String,
    /** How the client proves it may use the SNAP. */
    val credential: Credential = Credential.None,
    /** The server's SCION address, without a port: the port comes from [baseUrl]. */
    val target: String? = null,
    /** Which certificates this client accepts from the server. */
    val trust: Trust = Trust.SystemRoots,
)

/** The authority that mints tokens for Anapaya's own network. */
public const val ANAPAYA_AA: String = "https://auth.scion.anapaya.net"

/**
 * How a client proves to the SNAP that it may use the network.
 *
 * An API key is the long-lived secret. The authority mints tokens from it, each good for a day at
 * most, and the client renews them for as long as it runs.
 */
public sealed interface Credential {
    /** Nothing to prove. An endhost API on an appliance asks for no token. */
    public data object None : Credential

    /** One token, already minted. This is what `chat-dev` hands out. */
    public data class Token(val token: String) : Credential

    /** A key the client exchanges for tokens, and keeps exchanging. */
    public data class ApiKey(
        val key: String,
        val aaUrl: String = ANAPAYA_AA,
        val deviceId: String = "chat-android",
    ) : Credential
}

/** Which certificates a client accepts from the server. */
public sealed interface Trust {
    /** The anchors the device ships. */
    public data object SystemRoots : Trust

    /** One certificate, in place of the device's anchors. A self-signed server needs this. */
    public data class Pinned(val pem: String) : Trust

    /**
     * Accept any certificate.
     *
     * Every reply can come from anyone on the path. It exists so a demo can run against a
     * self-signed server without moving its certificate to the device first.
     */
    public data object Insecure : Trust
}
