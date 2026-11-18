// Copyright 2026 Anapaya Systems

package com.anapaya.chat.client

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import java.net.HttpURLConnection
import java.net.URL

/** The network info that `chat-dev` serves: what a client needs to reach the chat server. */
@Serializable
public data class DevNetwork(
    /** The endhost API that a client uses. */
    @SerialName("endhost_api_url") val endhostApiUrl: String,
    /** A token for this client only. Two clients with the same token evict each other. */
    @SerialName("auth_token") val authToken: String,
    /** The URL of the chat server. Its host is the name on the server's certificate. */
    @SerialName("base_url") val baseUrl: String,
    /** The server's SCION address. `chat-dev` has no TSAR records, so the client needs it. */
    @SerialName("target") val target: String,
    /** The server's certificate in PEM. An emulator cannot read the host's files. */
    @SerialName("ca_pem") val caPem: String,
) {
    /** The same network, as the SDK is configured with it. */
    public fun toScionConfig(): ScionConfig = ScionConfig(
        endhostApiUrl = endhostApiUrl,
        baseUrl = baseUrl,
        credential = Credential.Token(authToken),
        target = target,
        trust = Trust.Pinned(caPem),
    )

    public companion object {
        private val format = Json { ignoreUnknownKeys = true }

        /** What a control URL is pre-filled with: the emulator's view of the host's loopback. */
        public const val DEFAULT_CONTROL_URL: String = "http://10.0.2.2:8099"

        /** Reads the network info over plain HTTP, so it works when SCION does not. */
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
