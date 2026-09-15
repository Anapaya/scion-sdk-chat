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
    /** A certificate to trust in place of the device's anchors. */
    val certPem: String? = null,
)
