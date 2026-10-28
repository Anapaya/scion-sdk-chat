// Copyright 2026 Anapaya Systems

import Foundation

/// The network info that `chat-dev` serves: what a client needs to reach the chat server.
public struct DevNetwork: Decodable, Sendable {
    /// The endhost API that a client uses.
    public let endhostApiUrl: String
    /// A token for this client only. Two clients with the same token evict each other.
    public let authToken: String
    /// The URL of the chat server. Its host is the name on the server's certificate.
    public let baseUrl: String
    /// The server's SCION address. `chat-dev` has no TSAR records, so the client needs it.
    public let target: String
    /// The server's certificate in PEM. `chat-dev` makes a new one for each run.
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
            credential: .token(authToken),
            target: target,
            trust: .pinned(caPem))
    }

    /// Reads the network info over plain HTTP, so it works when SCION does not.
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
