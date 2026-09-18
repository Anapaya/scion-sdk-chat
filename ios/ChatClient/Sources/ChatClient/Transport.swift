// Copyright 2026 Anapaya Systems

import Foundation

/// One request, as the chat API asks for it.
public struct ChatRequest: Sendable {
    public let method: String
    /// Joined onto the server's base URL and its API prefix.
    public let path: String
    public let json: String?
    public let bearer: String?

    public init(method: String, path: String, json: String? = nil, bearer: String? = nil) {
        self.method = method
        self.path = path
        self.json = json
        self.bearer = bearer
    }
}

/// What came back.
public struct ChatReply: Sendable {
    public let status: Int
    public let body: String

    public init(status: Int, body: String) {
        self.status = status
        self.body = body
    }
}

/// Puts a request on the wire and brings the reply back. A protocol, so a test can fake it.
public protocol Transport: Sendable {
    func send(_ request: ChatRequest) async throws -> ChatReply
    func close() async
}
