// Copyright 2026 Anapaya Systems

package com.anapaya.chat.client

/**
 * What the SDK needs to reach a server over SCION.
 *
 * The first two are always known. The rest stand in for what a network cannot do for itself: a
 * token the SNAP underlay asks for, an address for a host no TSAR record answers for, and a
 * certificate for a server that signs its own. A production network leaves the three null, and the
 * SDK resolves the host and verifies it against the device's anchors.
 *
 * The same fields as `ScionConfig` in `chat-client-core`, so both clients describe SCION alike.
 */
public data class ScionConfig(
    /** The endhost API of the AS this client attaches to. Not the server's. */
    val endhostApiUrl: String,
    /** Where the server is. The host is the name its certificate is issued for. */
    val baseUrl: String,
    /** The token the SNAP underlay authenticates the tunnel with. */
    val snapToken: String? = null,
    /** The server's SCION address, without a port: the port comes from [baseUrl]. */
    val target: String? = null,
    /** A certificate to trust in place of the device's anchors. */
    val certPem: String? = null,
)
