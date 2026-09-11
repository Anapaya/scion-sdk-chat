// Copyright 2026 Anapaya Systems

package com.anapaya.chat.client

import android.content.Context
import com.anapaya.scion.http3.ScionAddress
import com.anapaya.scion.http3.ScionHttp3Client
import com.anapaya.scion.http3.ScionHttp3Exception
import com.anapaya.scion.http3.ScionHttp3Request
import com.anapaya.scion.http3.ScionHttp3RequestBody
import com.anapaya.scion.http3.TrustAnchors

/**
 * The whole of SCION, in one class.
 *
 * This is the only file in the project that imports `com.anapaya.scion.http3`. The `app` module
 * depends on this one and never on the SDK, so nothing above here can reach SCION by accident.
 *
 * What the SDK needs, and why:
 *
 * - **an endhost API**, which is how the client finds a SCION network at all;
 * - **a token**, which the SNAP underlay authenticates the tunnel with;
 * - **a trust anchor**, because the server signs its own certificate;
 * - **a target**, because this network has no TSAR records, so the URL's host is a name to check
 *   the certificate against rather than an address to resolve.
 */
public class ScionTransport(
    context: Context,
    private val network: DevNetwork,
) : Transport {
    private val target = runCatching { ScionAddress.parse(network.target) }
        .getOrElse { throw ChatError.Config("the target is not a SCION address: ${network.target}") }

    private val client: ScionHttp3Client =
        ScionHttp3Client
            .Builder(context)
            .endhostApi(network.endhostApiUrl)
            .authToken(network.authToken)
            // The server is its own authority, so the device's anchors would refuse it.
            .trust(TrustAnchors.pinned(network.caPem.toByteArray()))
            .build()

    override suspend fun send(request: ChatRequest): ChatReply {
        val built = ScionHttp3Request
            .Builder()
            .url("${network.baseUrl}/api/v1${request.path}")
            // Where to send the packets. The URL's host stays the name the certificate is issued
            // for, which is what the handshake checks.
            .target(target)
            .apply {
                request.bearer?.let { header("authorization", "Bearer $it") }
                when (request.json) {
                    null -> method(request.method, null)
                    else -> method(request.method, ScionHttp3RequestBody.json(request.json))
                }
            }
            .build()

        return try {
            // Closed as the sample does: a response holds a stream until it is read and released.
            client.newCall(built).execute().use { reply ->
                ChatReply(reply.code, reply.body.string())
            }
        } catch (error: ScionHttp3Exception) {
            throw failure(error)
        }
    }

    override fun close(): Unit = client.close()
}

/** Sorts an SDK failure into the one taxonomy the app knows, as `transport/scion.rs` does. */
internal fun failure(error: ScionHttp3Exception): ChatError.Transport {
    val kind = when (error) {
        is ScionHttp3Exception.Connectivity -> TransportFailure.Connectivity
        is ScionHttp3Exception.Resolution -> TransportFailure.Resolution
        is ScionHttp3Exception.Connect -> TransportFailure.Connect
        is ScionHttp3Exception.Tls -> TransportFailure.Tls
        is ScionHttp3Exception.StreamReset, is ScionHttp3Exception.ConnectionLimit ->
            TransportFailure.StreamReset
        is ScionHttp3Exception.BodyTooLarge -> TransportFailure.BodyTooLarge
        is ScionHttp3Exception.Timeout -> TransportFailure.Timeout
        is ScionHttp3Exception.Closed -> TransportFailure.Closed
        else -> TransportFailure.Protocol
    }

    return ChatError.Transport(kind, error.message ?: error.toString(), error)
}
