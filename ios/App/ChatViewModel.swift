// Copyright 2026 Anapaya Systems

import ChatClient
import Foundation

/// Which screen is showing. The flow is one way, except that an ended session goes back to signing in.
enum Screen {
    case connect
    case signIn
    case chat
}

/**
 Every call to the chat client, and the state the screens draw.

 The screens draw and report what was tapped; no view talks to a server, so there is one place to
 look for how the SDK is used.
 */
@MainActor
final class ChatViewModel: ObservableObject {
    @Published var screen: Screen = .connect
    @Published var controlUrl = DevNetwork.defaultControlUrl
    @Published var rooms: [Room] = []
    @Published var openRoomId: Int64?
    @Published var messages: [Message] = []
    @Published var unread: Set<Int64> = []
    @Published var username: String?
    /// Whether a call the reader asked for is still out. One at a time.
    @Published var pending = false
    /// Why the last read failed. Cleared by the next read that works.
    @Published var feedError: String?
    /// Why the last thing the reader asked for failed, held until they ask for something else.
    @Published var actionError: String?
    /// What to say in a banner. Taken once, then acknowledged.
    @Published var notice: String?
    /// Where the server is, once it is known.
    @Published var target: String?

    private var client: ChatClient?
    private var poll: Task<Void, Never>?
    /// The newest `seq` each room had when its listing was last seen, so unread is a change.
    private var seen: [Int64: Int64] = [:]

    var openRoom: Room? { rooms.first { $0.id == openRoomId } }

    // MARK: - Connecting

    /// Reads the network's description, builds a client, and proves the server is there.
    func connect() {
        ask {
            let network = try await DevNetwork.discover(controlUrl: self.controlUrl)
            let config = network.toScionConfig()
            let built = ChatClient(transport: try ScionTransport(config: config))
            // Building only parses configuration; nothing is dialled until a call is made. The
            // health check is what turns a wrong address into an error on this screen.
            try await built.health()

            self.client = built
            self.target = config.target ?? config.baseUrl
            self.screen = .signIn
        }
    }

    // MARK: - Signing in

    func register(username: String, password: String) {
        ask(refused: { why in self.notice = "Could not register \(username): \(why)" }) {
            try await self.requireClient().register(username: username, password: password)
            self.notice = "Registered \(username). Log in to continue."
        }
    }

    func logIn(username: String, password: String) {
        ask {
            let client = try self.requireClient()
            try await client.logIn(username: username, password: password)

            let rooms = try await client.rooms()
            // Seeded before anything is drawn, so the first frame is quiet.
            for room in rooms { self.seen[room.id] = room.latestSeq }

            self.rooms = rooms
            self.username = username
            self.openRoomId = rooms.first?.id
            self.messages = []
            self.screen = .chat

            if let open = rooms.first {
                self.messages = try await client.messagesNewest(room: open.id)
            }
            self.startPolling()
        }
    }

    // MARK: - Rooms and messages

    func openRoom(_ room: Room) {
        guard room.id != openRoomId else { return }
        openRoomId = room.id
        messages = []
        unread.remove(room.id)
        ask {
            self.messages = try await self.requireClient().messagesNewest(room: room.id)
        }
    }

    func createRoom(_ name: String) {
        ask {
            let room = try await self.requireClient().createRoom(name: name)
            self.openRoom(room)
        }
    }

    func send(_ body: String) {
        guard let room = openRoomId, !body.isEmpty else { return }
        ask { try await self.requireClient().send(room: room, body: body) }
    }

    // MARK: - Polling

    /// Re-reads the room list and the open room, so messages from elsewhere arrive.
    func startPolling() {
        poll?.cancel()
        poll = Task { [weak self] in
            while !Task.isCancelled {
                await self?.readOnce()
                try? await Task.sleep(nanoseconds: 2_000_000_000)
            }
        }
    }

    func stopPolling() {
        poll?.cancel()
        poll = nil
    }

    private func readOnce() async {
        guard let client else { return }
        do {
            let listing = try await client.rooms()
            for room in listing {
                let was = seen[room.id]
                // A room holding something newer than the last listing, and not the one being
                // read, is the only thing that marks unread.
                if let was, room.latestSeq > was, room.id != openRoomId {
                    unread.insert(room.id)
                }
                seen[room.id] = room.latestSeq
            }
            rooms = listing

            if let open = openRoomId {
                let batch: [Message]
                if let newest = messages.last?.seq {
                    batch = try await client.messagesAfter(room: open, after: newest)
                } else {
                    batch = try await client.messagesNewest(room: open)
                }
                if !batch.isEmpty { merge(batch) }
            }
            feedError = nil
        } catch ChatError.sessionExpired {
            signedOut()
        } catch {
            feedError = message(of: error)
        }
    }

    /// Adds a batch, keeping one row per `seq`.
    private func merge(_ batch: [Message]) {
        var held = messages
        let known = Set(held.map(\.seq))
        held.append(contentsOf: batch.filter { !known.contains($0.seq) })
        messages = held.sorted { $0.seq < $1.seq }
    }

    // MARK: - Plumbing

    private func requireClient() throws -> ChatClient {
        guard let client else { throw ChatError.notLoggedIn }
        return client
    }

    private func signedOut() {
        stopPolling()
        username = nil
        screen = .signIn
        actionError = "the session has ended; sign in again"
    }

    private func message(of error: Error) -> String {
        (error as? ChatError)?.errorDescription ?? error.localizedDescription
    }

    /// Runs one call, refusing a second while one is out, and reporting what went wrong.
    private func ask(
        refused: @escaping (String) -> Void = { _ in },
        _ work: @escaping () async throws -> Void
    ) {
        guard !pending else { return }
        pending = true
        actionError = nil

        Task {
            do {
                try await work()
            } catch ChatError.sessionExpired {
                signedOut()
            } catch {
                let why = message(of: error)
                actionError = why
                refused(why)
            }
            pending = false
        }
    }
}
