// Copyright 2026 Anapaya Systems

import Foundation
import ScionHTTP3

/// The whole of SCION, in one type. The only file that imports `ScionHTTP3`.
public final class ScionTransport: Transport {
    private let config: ScionConfig
    private let client: ScionHttp3Client

    /// Where to send the packets, for a network that publishes no TSAR record. Null in production.
    private let address: ScionAddress?

    /// Renews the token for as long as this transport lives. Nil when nothing expires.
    private let renewal: Task<Void, Never>?

    /// Builds the client, minting a token first when the configuration carries a key.
    ///
    /// Asynchronous because the authority is asked here: the SDK takes a token and cannot be given
    /// one later, so the exchange has to finish before the client exists.
    public init(config: ScionConfig) async throws {
        self.config = config

        if let target = config.target {
            do {
                address = try ScionAddress(target)
            } catch {
                throw ChatError.config("the target is not a SCION address: \(target)")
            }
        } else {
            address = nil
        }

        let minted: MintedToken?
        switch config.credential {
        case .none: minted = nil
        case .token(let token): minted = MintedToken(token: token, expiresAt: .distantFuture)
        case .apiKey(let auth): minted = try await mint(auth)
        }

        var settings = ScionHttp3Client.Configuration(
            endhostApi: config.endhostApiUrl,
            authToken: minted?.token)
        if let address {
            // The URL's host stays the name the certificate must carry. The lookup only.
            settings.dnsOverrides = [try host(of: config.baseUrl): [address]]
        }
        switch config.trust {
        case .systemRoots:
            break
        case .pinned(let pem):
            do {
                settings.trust = try .pinned(Data(pem.utf8))
            } catch {
                throw ChatError.config("the certificate is not readable PEM")
            }
        case .insecure:
            settings.trust = .insecureNoVerify
        }

        do {
            client = try ScionHttp3Client(configuration: settings)
        } catch {
            throw ChatError.config("the SCION client could not be built: \(error)")
        }

        if case .apiKey(let auth) = config.credential, let first = minted {
            renewal = Self.renew(client: client, auth: auth, first: first)
        } else {
            renewal = nil
        }
    }

    /// Mints a fresh token before each one expires, and hands it to the client.
    private static func renew(
        client: ScionHttp3Client, auth: ApiKeyAuth, first: MintedToken
    ) -> Task<Void, Never> {
        Task {
            var current = first
            while !Task.isCancelled {
                let wait = max(
                    current.expiresAt.timeIntervalSinceNow - renewEarly, renewRetry)
                try? await Task.sleep(nanoseconds: UInt64(wait * 1_000_000_000))
                if Task.isCancelled { return }

                // A failure leaves the old token in place and the loop waits out the retry gap.
                // It is still good for a little longer, so there is nothing to report yet.
                guard let next = try? await mint(auth) else { continue }

                current = next
                try? client.setAuthToken(next.token)
            }
        }
    }

    public func send(_ request: ChatRequest) async throws -> ChatReply {
        var built = ScionHttp3Request(url: "\(config.baseUrl)/api/v1\(request.path)")
        built.method = ScionHttp3Request.Method(request.method)
        if let bearer = request.bearer {
            built.headers.add("authorization", "Bearer \(bearer)")
        }
        if let json = request.json {
            built.body = .json(json)
        }

        do {
            let response = try await client.execute(built)
            return ChatReply(status: response.code, body: try await response.body.string())
        } catch let error as ScionHttp3Error {
            throw failure(error)
        }
    }

    public func close() async {
        renewal?.cancel()
        await client.shutdown()
    }
}

/// The name in a URL, which is what a DNS override is keyed on.
private func host(of url: String) throws -> String {
    guard let host = URLComponents(string: url)?.host else {
        throw ChatError.config("the server URL has no host: \(url)")
    }
    return host
}

/// How long before a token expires the next one is asked for.
private let renewEarly: TimeInterval = 60

/// How long to wait before trying again, and the shortest gap between attempts.
private let renewRetry: TimeInterval = 30

/// Sorts an SDK failure into the one taxonomy the app knows.
func failure(_ error: ScionHttp3Error) -> ChatError {
    let kind: TransportFailure
    switch error {
    case .connectivity: kind = .connectivity
    case .resolution: kind = .resolution
    case .connect: kind = .connect
    case .tls: kind = .tls
    case .streamReset: kind = .streamReset
    case .timeout: kind = .timeout
    case .bodyTooLarge: kind = .bodyTooLarge
    default: kind = .protocolError
    }

    return .transport(kind, error.detail)
}
