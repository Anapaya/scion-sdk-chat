// Copyright 2026 Anapaya Systems

import Foundation

/// Why a transport could not carry a request.
public enum TransportFailure: Sendable, Equatable {
    case connectivity
    case resolution
    case connect
    case tls
    case streamReset
    case timeout
    case bodyTooLarge
    case protocolError
}

/// Everything a call can fail with.
public enum ChatError: Error, Sendable, Equatable {
    /// A setting cannot be right, found before anything is sent.
    case config(String)
    /// The request never produced a response.
    case transport(TransportFailure, String)
    /// A reply arrived that this client cannot read.
    case protocolError(String)
    /// The server refused, and said why.
    case api(status: Int, code: String, message: String)
    /// A call that needs a session was made without one.
    case notLoggedIn
    /// The session ended. The reader has to sign in again.
    case sessionExpired
}

extension ChatError: LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .config(let detail):
            return detail
        case .transport(_, let detail):
            return detail
        case .protocolError(let detail):
            return detail
        case .api(let status, let code, let message):
            return "\(code) (\(status)): \(message)"
        case .notLoggedIn:
            return "not logged in"
        case .sessionExpired:
            return "the session has ended"
        }
    }
}
