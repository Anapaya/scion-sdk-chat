// Copyright 2026 Anapaya Systems

import Foundation

/**
 Where a `chat-dev` network is, and how to be trusted by it.

 A deployed app knows all of this before it is built. `chat-dev` decides it at startup — the
 endhost APIs take whatever ports are free, the certificate is generated, and a token is minted
 per read — so this asks.
 */
public struct DevNetwork: Decodable, Sendable {
    /// The endhost API of the AS a client attaches to. Not the server's.
    public let endhostApiUrl: String
    /// This reader's own token. Two clients sharing one evict each other.
    public let authToken: String
    /// Where the server is. The host is the name its certificate is issued for.
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
            certPem: caPem)
    }

    /**
     Reads the description `chat-dev` serves.

     Over plain HTTP, never over SCION: a client that cannot reach the network still has to be able
     to learn why.
     */
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
