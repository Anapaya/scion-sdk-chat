// Copyright 2026 Anapaya Systems

package com.anapaya.chat.client

import java.io.ByteArrayOutputStream
import java.net.HttpURLConnection
import java.net.URL
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * A token, and the moment it stops being one.
 *
 * @property token what the SNAP is given.
 * @property expiresAtMillis when it expires, on this device's clock.
 */
public data class MintedToken(val token: String, val expiresAtMillis: Long)

/**
 * Trades an API key for a token at the authority.
 *
 * The SDK takes a token and nothing else, so the exchange happens here and the result is handed to
 * it. This call does not go over SCION: it is ordinary HTTPS, made by the platform.
 */
internal suspend fun mint(auth: Credential.ApiKey): MintedToken = withContext(Dispatchers.IO) {
    val body = protobuf {
        string(1, auth.key)
        string(2, auth.deviceId)
        varint(3, TOKEN_VALIDITY_SECONDS)
    }

    val connection = (URL("${auth.aaUrl.trimEnd('/')}$AUTHENTICATE_BY_KEY").openConnection()
        as HttpURLConnection).apply {
        requestMethod = "POST"
        // The authority speaks protobuf and refuses JSON.
        setRequestProperty("content-type", "application/proto")
        connectTimeout = CONNECT_TIMEOUT_MILLIS
        readTimeout = READ_TIMEOUT_MILLIS
        doOutput = true
    }

    val reply = try {
        connection.outputStream.use { it.write(body) }
        when (val code = connection.responseCode) {
            in 200..299 -> connection.inputStream.use { it.readBytes() }
            else -> {
                val detail = connection.errorStream?.use { it.readBytes() }?.decodeToString().orEmpty()
                throw ChatError.Config(
                    if (code == 401) "the authority refused this API key: $detail"
                    else "the authority answered $code: $detail",
                )
            }
        }
    } catch (error: java.io.IOException) {
        throw ChatError.Config("the authority at ${auth.aaUrl} is not answering: ${error.message}")
    } finally {
        connection.disconnect()
    }

    val token = fieldOne(reply)
        ?: throw ChatError.Config("the authority answered without a token")

    MintedToken(token, expiryOf(token))
}

/** When a token stops being valid, from the `exp` its payload carries. */
private fun expiryOf(token: String): Long {
    val payload = token.split('.').getOrNull(1)
        ?: throw ChatError.Config("the authority's token is not a JWT")
    val claims = try {
        android.util.Base64.decode(
            payload,
            android.util.Base64.URL_SAFE or android.util.Base64.NO_PADDING or
                android.util.Base64.NO_WRAP,
        ).decodeToString()
    } catch (error: IllegalArgumentException) {
        throw ChatError.Config("the authority's token is not readable: ${error.message}")
    }

    // Read, never verified: the SNAP is what checks the signature. This only decides when to ask
    // for the next one.
    val seconds = Regex("\"exp\"\\s*:\\s*(\\d+)").find(claims)?.groupValues?.get(1)?.toLongOrNull()
        ?: throw ChatError.Config("the authority's token names no expiry")

    return seconds * 1000
}

/** The first length-delimited field of a message, which is where the token sits. */
private fun fieldOne(message: ByteArray): String? {
    if (message.isEmpty() || message[0] != 0x0a.toByte()) return null

    var at = 1
    var length = 0
    var shift = 0
    while (at < message.size) {
        val byte = message[at].toInt()
        at += 1
        length = length or ((byte and 0x7f) shl shift)
        if (byte and 0x80 == 0) break
        shift += 7
    }

    return if (at + length <= message.size) {
        message.copyOfRange(at, at + length).decodeToString()
    } else {
        null
    }
}

/** Builds a protobuf message field by field. */
internal fun protobuf(fields: ProtobufWriter.() -> Unit): ByteArray =
    ProtobufWriter().apply(fields).bytes()

/** The few pieces of the wire format this exchange needs. */
internal class ProtobufWriter {
    private val out = ByteArrayOutputStream()

    /** A length-delimited field, which is how a string is written. */
    fun string(number: Int, value: String) {
        val data = value.encodeToByteArray()
        out.write(number shl 3 or 2)
        varint(data.size.toLong())
        out.write(data)
    }

    /** A varint field. */
    fun varint(number: Int, value: Long) {
        out.write(number shl 3)
        varint(value)
    }

    private fun varint(value: Long) {
        var rest = value
        while (true) {
            val byte = (rest and 0x7f).toInt()
            rest = rest ushr 7
            if (rest == 0L) {
                out.write(byte)
                return
            }
            out.write(byte or 0x80)
        }
    }

    fun bytes(): ByteArray = out.toByteArray()
}

/** The route the authority answers on. */
private const val AUTHENTICATE_BY_KEY = "/anapaya.aa.v1.AuthService/AuthenticateByKey"

/** How long a token is asked for. The authority caps this at a day. */
private const val TOKEN_VALIDITY_SECONDS = 86_400L

private const val CONNECT_TIMEOUT_MILLIS = 10_000
private const val READ_TIMEOUT_MILLIS = 15_000
