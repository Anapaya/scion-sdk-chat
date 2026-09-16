// Copyright 2026 Anapaya Systems

import Foundation

/// The chat API, over whatever ``Transport`` it is given. Holds the session token.
public actor ChatClient {
    /// How many messages a page holds.
    public static let page = 50

    private let transport: Transport
    private var token: String?

    /// Who is logged in.
    public private(set) var username: String?

    public init(transport: Transport) {
        self.transport = transport
    }

    public func health() async throws {
        _ = try await call("GET", "/healthz", authenticated: false)
    }

    public func serverInfo() async throws -> ServerInfo {
        try decode(await call("GET", "/server", authenticated: false))
    }

    /// Creates the account. Does not log in: the server keeps the two apart.
    public func register(username: String, password: String) async throws {
        let body = RegisterRequest(password: password, username: username)
        _ = try await call("POST", "/register", json: encode(body), authenticated: false)
    }

    public func logIn(username: String, password: String) async throws {
        let body = LoginRequest(password: password, username: username)
        let reply: LoginResponse = try decode(
            await call("POST", "/login", json: encode(body), authenticated: false))

        token = reply.token
        self.username = username
    }

    public func rooms() async throws -> [Room] {
        let reply: RoomsResponse = try decode(await call("GET", "/rooms"))
        return reply.rooms
    }

    public func createRoom(name: String) async throws -> Room {
        try decode(await call("POST", "/rooms", json: encode(CreateRoomRequest(name: name))))
    }

    public func messagesNewest(room: Int64, limit: Int = page) async throws -> [Message] {
        let reply: MessagesResponse = try decode(
            await call("GET", "/rooms/\(room)/messages?limit=\(limit)"))
        return reply.messages
    }

    public func messagesAfter(room: Int64, after: Int64, limit: Int = page) async throws -> [Message] {
        let reply: MessagesResponse = try decode(
            await call("GET", "/rooms/\(room)/messages?after_seq=\(after)&limit=\(limit)"))
        return reply.messages
    }

    public func send(room: Int64, body: String) async throws {
        _ = try await call(
            "POST", "/rooms/\(room)/messages", json: encode(PostMessageRequest(body: body)))
    }

    public func close() async {
        await transport.close()
    }

    /// Sends, and turns anything that is not a success into a ``ChatError``.
    private func call(
        _ method: String,
        _ path: String,
        json: String? = nil,
        authenticated: Bool = true
    ) async throws -> String {
        var bearer: String?
        if authenticated {
            guard let held = token else { throw ChatError.notLoggedIn }
            bearer = held
        }

        let reply = try await transport.send(
            ChatRequest(method: method, path: path, json: json, bearer: bearer))
        if (200...299).contains(reply.status) {
            return reply.body
        }

        let refusal = Self.refusal(status: reply.status, body: reply.body)
        // The token is gone, not refused, so retrying with it cannot work.
        if refusal == .sessionExpired {
            token = nil
            username = nil
        }

        throw refusal
    }

    private func encode<T: Encodable>(_ value: T) throws -> String {
        guard let json = try? JSONEncoder().encode(value),
              let text = String(data: json, encoding: .utf8)
        else {
            throw ChatError.protocolError("a request could not be encoded")
        }
        return text
    }

    private func decode<T: Decodable>(_ body: String) throws -> T {
        do {
            return try JSONDecoder().decode(T.self, from: Data(body.utf8))
        } catch {
            throw ChatError.protocolError("a reply could not be decoded: \(error)")
        }
    }

    private static func refusal(status: Int, body: String) -> ChatError {
        guard let envelope = try? JSONDecoder().decode(ErrorEnvelope.self, from: Data(body.utf8))
        else {
            return .protocolError("\(status) carried no error envelope")
        }

        switch envelope.error.code {
        case "expired_token", "unauthorized":
            return .sessionExpired
        default:
            return .api(
                status: status, code: envelope.error.code, message: envelope.error.message)
        }
    }
}

/// The body of every failing response. `code` is a `String`, so a code added later still decodes.
struct ErrorEnvelope: Decodable {
    let error: ApiFailure

    struct ApiFailure: Decodable {
        let code: String
        let message: String
    }
}
