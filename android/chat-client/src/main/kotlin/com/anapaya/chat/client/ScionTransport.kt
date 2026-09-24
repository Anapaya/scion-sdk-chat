// Copyright 2026 Anapaya Systems

package com.anapaya.chat.client

import android.content.Context
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import com.anapaya.scion.http3.ScionAddress
import com.anapaya.scion.http3.ScionHttp3Client
import com.anapaya.scion.http3.ScionHttp3Exception
import com.anapaya.scion.http3.ScionHttp3Request
import com.anapaya.scion.http3.ScionHttp3RequestBody
import com.anapaya.scion.http3.TrustAnchors

/** The whole of SCION, in one class. The only file that imports `com.anapaya.scion.http3`. */
public class ScionTransport private constructor(
    private val config: ScionConfig,
    private val client: ScionHttp3Client,
    /** Where to send the packets, for a network with no TSAR record. Null in production. */
    private val address: ScionAddress?,
    /** Renews the token for as long as this transport lives. Null when nothing expires. */
    private val renewal: Job?,
) : Transport {

    public companion object {
        /**
         * Builds the client, minting a token first when the configuration carries a key.
         *
         * Suspending because the authority is asked here: the SDK takes a token and cannot be
         * given one later, so the exchange has to finish before the client exists.
         */
        public suspend fun open(context: Context, config: ScionConfig): ScionTransport {
            val address = config.target?.let { target ->
                runCatching { ScionAddress.parse(target) }
                    .getOrElse { throw ChatError.Config("the target is not a SCION address: $target") }
            }

            val minted = when (val credential = config.credential) {
                is Credential.None -> null
                is Credential.Token -> MintedToken(credential.token, Long.MAX_VALUE)
                is Credential.ApiKey -> mint(credential)
            }

            val client = ScionHttp3Client
                .Builder(context)
                .endhostApi(config.endhostApiUrl)
                .apply { minted?.let { authToken(it.token) } }
                .trust(
                    when (val trust = config.trust) {
                        is Trust.SystemRoots -> TrustAnchors.systemDefault()
                        is Trust.Pinned -> TrustAnchors.pinned(trust.pem.toByteArray())
                        is Trust.Insecure -> TrustAnchors.insecureNoVerify()
                    },
                )
                .build()

            val credential = config.credential
            val renewal = if (credential is Credential.ApiKey && minted != null) {
                renew(client, credential, minted)
            } else {
                null
            }

            return ScionTransport(config, client, address, renewal)
        }

        /** Mints a fresh token before each one expires, and hands it to the client. */
        private fun renew(
            client: ScionHttp3Client,
            auth: Credential.ApiKey,
            first: MintedToken,
        ): Job = CoroutineScope(Dispatchers.IO).launch {
            var current = first
            while (isActive) {
                val wait = current.expiresAtMillis - System.currentTimeMillis() - RENEW_EARLY_MILLIS
                delay(wait.coerceAtLeast(RENEW_RETRY_MILLIS))

                // A failure leaves the old token in place and the loop waits out the retry gap.
                // It is still good for a little longer, so there is nothing to report yet.
                val next = runCatching { mint(auth) }.getOrNull()
                if (next != null) {
                    current = next
                    client.setAuthToken(next.token)
                }
            }
        }

        /** How long before a token expires the next one is asked for. */
        private const val RENEW_EARLY_MILLIS = 60_000L

        /** How long to wait before trying again, and the shortest gap between attempts. */
        private const val RENEW_RETRY_MILLIS = 30_000L
    }

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

    override fun close() {
        renewal?.cancel()
        client.close()
    }
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
