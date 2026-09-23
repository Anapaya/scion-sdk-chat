// Copyright 2026 Anapaya Systems

package com.anapaya.chat.client

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import java.net.HttpURLConnection
import java.net.URL

/** Where a `chat-dev` network is, and how to be trusted by it. A deployed app is told this. */
@Serializable
public data class DevNetwork(
    /** The endhost API of the AS a client attaches to. */
    @SerialName("endhost_api_url") val endhostApiUrl: String,
    /** This reader's own token. Two clients sharing one evict each other. */
    @SerialName("auth_token") val authToken: String,
    /** Where the server is. Its host is the name the certificate is issued for. */
    @SerialName("base_url") val baseUrl: String,
    /** The server's SCION address, without a port. */
    @SerialName("target") val target: String,
    /** The certificate to pin, inline because an emulator cannot read the host's filesystem. */
    @SerialName("ca_pem") val caPem: String,
) {
    /** The same network, as the SDK is configured with it. */
    public fun toScionConfig(): ScionConfig = ScionConfig(
        endhostApiUrl = endhostApiUrl,
        baseUrl = baseUrl,
        snapToken = authToken,
        target = target,
        trust = Trust.Pinned(caPem),
    )

    public companion object {
        private val format = Json { ignoreUnknownKeys = true }

        /** What a control URL is pre-filled with: the emulator's view of the host's loopback. */
        public const val DEFAULT_CONTROL_URL: String = "http://10.0.2.2:8099"

        /** Reads the description `chat-dev` serves, over plain HTTP: SCION may be what is broken. */
        public suspend fun discover(controlUrl: String): DevNetwork = withContext(Dispatchers.IO) {
            val url = URL("${controlUrl.trimEnd('/')}/info")
            val connection = (url.openConnection() as HttpURLConnection).apply {
                connectTimeout = TIMEOUT_MILLIS
                readTimeout = TIMEOUT_MILLIS
            }

            val text = try {
                connection.inputStream.bufferedReader().use { it.readText() }
            } catch (error: Exception) {
                throw ChatError.Config("no chat-dev at $url: ${error.message}")
            } finally {
                connection.disconnect()
            }

            try {
                format.decodeFromString<DevNetwork>(text)
            } catch (error: Exception) {
                throw ChatError.Config("$url did not answer with a network: ${error.message}")
            }
        }

        private const val TIMEOUT_MILLIS = 5_000
    }
}
