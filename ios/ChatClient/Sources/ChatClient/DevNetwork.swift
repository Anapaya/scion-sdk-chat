// Copyright 2026 Anapaya Systems

import Foundation

/// Where a `chat-dev` network is, and how to be trusted by it. A deployed app is told this.
public struct DevNetwork: Decodable, Sendable {
    /// The endhost API of the AS a client attaches to.
    public let endhostApiUrl: String
    /// This reader's own token. Two clients sharing one evict each other.
    public let authToken: String
    /// Where the server is. Its host is the name the certificate is issued for.
    public let baseUrl: String
    /// The server's SCION address, without a port.
    public let target: String
    /// The certificate to pin, inline because it is generated per run.
    public let caPem: String

    enum CodingKeys: String, CodingKey {
        case endhostApiUrl = "endhost_api_url"
        case authToken = "auth_token"
        case baseUrl = "base_url"
        case target
        case caPem = "ca_pem"
    }

    /// The simulator shares the Mac's network, so a loopback address here is the Mac's.
    public static let defaultControlUrl = "http://127.0.0.1:8099"

    /// The same network, as the SDK is configured with it.
    public func toScionConfig() -> ScionConfig {
        ScionConfig(
            endhostApiUrl: endhostApiUrl,
            baseUrl: baseUrl,
            snapToken: authToken,
            target: target,
            trust: .pinned(caPem))
    }

    /// Reads the description `chat-dev` serves, over plain HTTP: SCION may be what is broken.
    public static func discover(controlUrl: String) async throws -> DevNetwork {
        let trimmed = controlUrl.hasSuffix("/") ? String(controlUrl.dropLast()) : controlUrl
        guard let url = URL(string: trimmed + "/info") else {
            throw ChatError.config("not a URL: \(controlUrl)")
        }

        let data: Data
        do {
            (data, _) = try await URLSession.shared.data(from: url)
        } catch {
            throw ChatError.config("no chat-dev at \(url): \(error.localizedDescription)")
        }

        do {
            return try JSONDecoder().decode(DevNetwork.self, from: data)
        } catch {
            throw ChatError.config("\(url) did not answer with a network")
        }
    }
}
