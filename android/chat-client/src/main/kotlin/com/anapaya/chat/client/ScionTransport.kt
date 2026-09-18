// Copyright 2026 Anapaya Systems

package com.anapaya.chat.client

import android.content.Context
import com.anapaya.scion.http3.ScionAddress
import com.anapaya.scion.http3.ScionHttp3Client
import com.anapaya.scion.http3.ScionHttp3Exception
import com.anapaya.scion.http3.ScionHttp3Request
import com.anapaya.scion.http3.ScionHttp3RequestBody
import com.anapaya.scion.http3.TrustAnchors

/** The whole of SCION, in one class. The only file that imports `com.anapaya.scion.http3`. */
public class ScionTransport(
    context: Context,
    private val config: ScionConfig,
) : Transport {
    /** Where to send the packets, for a network that publishes no TSAR record. Null in production. */
    private val address = config.target?.let { target ->
        runCatching { ScionAddress.parse(target) }
            .getOrElse { throw ChatError.Config("the target is not a SCION address: $target") }
    }

    private val client: ScionHttp3Client =
        ScionHttp3Client
            .Builder(context)
            .endhostApi(config.endhostApiUrl)
            .apply { config.snapToken?.let { authToken(it) } }
            .trust(
                config.certPem
                    ?.let { TrustAnchors.pinned(it.toByteArray()) }
                    ?: TrustAnchors.systemDefault(),
            )
            .build()

    override suspend fun send(request: ChatRequest): ChatReply {
        val built = ScionHttp3Request
            .Builder()
            .url("${config.baseUrl}/api/v1${request.path}")
            .apply {
                // The URL's host stays the name the certificate must carry. The lookup only.
                address?.let { target(it) }
            }
            .apply {
                request.bearer?.let { header("authorization", "Bearer $it") }
                when (request.json) {
                    null -> method(request.method, null)
                    else -> method(request.method, ScionHttp3RequestBody.json(request.json))
                }
            }
            .build()

        return try {
            // A response holds a stream until it is read and released.
            client.newCall(built).execute().use { reply ->
                ChatReply(reply.code, reply.body.string())
            }
        } catch (error: ScionHttp3Exception) {
            throw failure(error)
        }
    }

    override fun close(): Unit = client.close()
}

/** Sorts an SDK failure into the one taxonomy the app knows. */
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
