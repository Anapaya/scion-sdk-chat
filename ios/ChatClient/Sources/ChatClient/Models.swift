// Copyright 2026 Anapaya Systems

import Foundation

/// A room, as the last listing described it.
public struct Room: Codable, Sendable, Identifiable, Equatable {
    public let id: Int64
    public let name: String
    /// The `seq` of the newest message in it, which is what says a room has something new.
    public let latestSeq: Int64

    enum CodingKeys: String, CodingKey {
        case id, name
        case latestSeq = "latest_seq"
    }
}

/// Something somebody said.
public struct Message: Codable, Sendable, Identifiable, Equatable {
    /// Assigned server-wide and strictly increasing, which makes it the identity to merge on.
    public let seq: Int64
    public let username: String
    public let body: String
    public let postedAt: Int64

    public var id: Int64 { seq }

    enum CodingKeys: String, CodingKey {
        case seq, username, body
        case postedAt = "posted_at"
    }
}

/// What the server says about itself.
public struct ServerInfo: Codable, Sendable, Equatable {
    public let version: String
    public let maxAccounts: Int
    public let maxRooms: Int
    public let maxMessageBytes: Int
    public let tokenValiditySeconds: Int
    public let isdAs: String?

    enum CodingKeys: String, CodingKey {
        case version
        case maxAccounts = "max_accounts"
        case maxRooms = "max_rooms"
        case maxMessageBytes = "max_message_bytes"
        case tokenValiditySeconds = "token_validity_seconds"
        case isdAs = "isd_as"
    }
}

struct LoginResponse: Decodable {
    let token: String
    let expiresAt: Int64

    enum CodingKeys: String, CodingKey {
        case token
        case expiresAt = "expires_at"
    }
}

struct RoomsResponse: Decodable {
    let rooms: [Room]
}

struct MessagesResponse: Decodable {
    let messages: [Message]
}

struct Credentials: Encodable {
    let username: String
    let password: String
}

struct CreateRoomRequest: Encodable {
    let name: String
}

struct PostMessageRequest: Encodable {
    let body: String
}

/// The body of every failing response.
///
/// `code` is a `String`: the server's `ErrorCode` carries a catch-all, so a code added after this
/// build ships still decodes.
struct ErrorEnvelope: Decodable {
    let error: ApiFailure

    struct ApiFailure: Decodable {
        let code: String
        let message: String
    }
}
