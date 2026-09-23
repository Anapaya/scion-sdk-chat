// Copyright 2026 Anapaya Systems

import Foundation
import ScionHTTP3

/// The whole of SCION, in one type. The only file that imports `ScionHTTP3`.
public final class ScionTransport: Transport {
    private let config: ScionConfig
    private let client: ScionHttp3Client

    /// Where to send the packets, for a network that publishes no TSAR record. Null in production.
    private let address: ScionAddress?

    public init(config: ScionConfig) throws {
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

        var settings = ScionHttp3Client.Configuration(
            endhostApi: config.endhostApiUrl,
            authToken: config.snapToken)
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
    }

    public func send(_ request: ChatRequest) async throws -> ChatReply {
        var built = ScionHttp3Request(url: "\(config.baseUrl)/api/v1\(request.path)")
        built.method = ScionHttp3Request.Method(request.method)
        // The URL's host stays the name the certificate must carry. The lookup only.
        if let address { built.targets = [address] }
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
        await client.shutdown()
    }
}

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
