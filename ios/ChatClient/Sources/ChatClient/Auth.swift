// Copyright 2026 Anapaya Systems

import Foundation

/// A token, and the moment it stops being one.
public struct MintedToken: Sendable, Equatable {
    /// What the SNAP is given.
    public let token: String
    /// When it expires, on this device's clock.
    public let expiresAt: Date
}

/// Trades an API key for a token at the authority.
///
/// The SDK takes a token and nothing else, so the exchange happens here and the result is handed to
/// it. This call does not go over SCION: it is ordinary HTTPS, made by the platform.
func mint(_ auth: ApiKeyAuth) async throws -> MintedToken {
    guard let url = URL(string: auth.aaUrl.hasSuffix("/")
        ? String(auth.aaUrl.dropLast()) + authenticateByKey
        : auth.aaUrl + authenticateByKey)
    else {
        throw ChatError.config("the authority's address is not a URL: \(auth.aaUrl)")
    }

    var request = URLRequest(url: url)
    request.httpMethod = "POST"
    // The authority speaks protobuf and refuses JSON.
    request.setValue("application/proto", forHTTPHeaderField: "content-type")
    request.timeoutInterval = requestTimeout
    request.httpBody = protobuf { writer in
        writer.string(1, auth.key)
        writer.string(2, auth.deviceId)
        writer.varint(3, tokenValiditySeconds)
    }

    let data: Data
    let response: URLResponse
    do {
        (data, response) = try await URLSession.shared.data(for: request)
    } catch {
        throw ChatError.config(
            "the authority at \(auth.aaUrl) is not answering: \(error.localizedDescription)")
    }

    if let http = response as? HTTPURLResponse, !(200...299).contains(http.statusCode) {
        let detail = String(data: data, encoding: .utf8) ?? ""
        throw ChatError.config(
            http.statusCode == 401
                ? "the authority refused this API key: \(detail)"
                : "the authority answered \(http.statusCode): \(detail)")
    }

    guard let token = fieldOne(data) else {
        throw ChatError.config("the authority answered without a token")
    }

    return MintedToken(token: token, expiresAt: try expiry(of: token))
}

/// When a token stops being valid, from the `exp` its payload carries.
private func expiry(of token: String) throws -> Date {
    let parts = token.split(separator: ".")
    guard parts.count >= 2 else {
        throw ChatError.config("the authority's token is not a JWT")
    }

    var payload = String(parts[1]).replacingOccurrences(of: "-", with: "+")
        .replacingOccurrences(of: "_", with: "/")
    while payload.count % 4 != 0 { payload.append("=") }

    guard let decoded = Data(base64Encoded: payload),
        // Read, never verified: the SNAP is what checks the signature. This only decides when to
        // ask for the next one.
        let claims = try? JSONSerialization.jsonObject(with: decoded) as? [String: Any],
        let seconds = claims["exp"] as? Double
    else {
        throw ChatError.config("the authority's token names no expiry")
    }

    return Date(timeIntervalSince1970: seconds)
}

/// The first length-delimited field of a message, which is where the token sits.
private func fieldOne(_ message: Data) -> String? {
    let bytes = [UInt8](message)
    guard bytes.first == 0x0a else { return nil }

    var at = 1
    var length = 0
    var shift = 0
    while at < bytes.count {
        let byte = Int(bytes[at])
        at += 1
        length |= (byte & 0x7f) << shift
        if byte & 0x80 == 0 { break }
        shift += 7
    }

    guard at + length <= bytes.count else { return nil }
    return String(bytes: bytes[at..<(at + length)], encoding: .utf8)
}

/// Builds a protobuf message field by field.
func protobuf(_ fields: (inout ProtobufWriter) -> Void) -> Data {
    var writer = ProtobufWriter()
    fields(&writer)
    return writer.bytes
}

/// The few pieces of the wire format this exchange needs.
struct ProtobufWriter {
    private(set) var bytes = Data()

    /// A length-delimited field, which is how a string is written.
    mutating func string(_ number: Int, _ value: String) {
        let data = Data(value.utf8)
        bytes.append(UInt8(number << 3 | 2))
        varint(UInt64(data.count))
        bytes.append(data)
    }

    /// A varint field.
    mutating func varint(_ number: Int, _ value: UInt64) {
        bytes.append(UInt8(number << 3))
        varint(value)
    }

    private mutating func varint(_ value: UInt64) {
        var rest = value
        while true {
            let byte = UInt8(rest & 0x7f)
            rest >>= 7
            if rest == 0 {
                bytes.append(byte)
                return
            }
            bytes.append(byte | 0x80)
        }
    }
}

/// The route the authority answers on.
private let authenticateByKey = "/anapaya.aa.v1.AuthService/AuthenticateByKey"

/// How long a token is asked for. The authority caps this at a day.
private let tokenValiditySeconds: UInt64 = 86_400

private let requestTimeout: TimeInterval = 15
