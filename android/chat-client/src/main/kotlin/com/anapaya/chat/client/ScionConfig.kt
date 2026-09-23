// Copyright 2026 Anapaya Systems

package com.anapaya.chat.client

/** What the SDK needs to reach a server over SCION. */
public data class ScionConfig(
    /** The endhost API of the AS this client attaches to. */
    val endhostApiUrl: String,
    /** Where the server is. Its host is the name the certificate is issued for. */
    val baseUrl: String,
    /** The token the SNAP underlay authenticates the tunnel with. */
    val snapToken: String? = null,
    /** The server's SCION address, without a port: the port comes from [baseUrl]. */
    val target: String? = null,
    /** Which certificates this client accepts from the server. */
    val trust: Trust = Trust.SystemRoots,
)

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
