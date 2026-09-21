// Copyright 2026 Anapaya Systems

import ChatClient
import Foundation

/// How often the open room is re-read.
private let messagesInterval: UInt64 = 1_000_000_000

/// The room list is less urgent than the open room.
private let roomsInterval: UInt64 = 2_000_000_000

/// Which screen is showing.
enum Screen {
    case connect
    /// Connect, with the configuration typed out.
    case manual
    case signIn
    case chat
}

/// A SCION configuration as it is typed. A blank field means the network answers for it.
struct ManualForm: Equatable {
    var endhostApiUrl = ""
    var baseUrl = ""
    var snapToken = ""
    var target = ""
    var certPem = ""

    func toScionConfig() -> ScionConfig {
        ScionConfig(
            endhostApiUrl: trimmed(endhostApiUrl) ?? "",
            baseUrl: trimmed(baseUrl) ?? "",
            snapToken: trimmed(snapToken),
            target: trimmed(target),
            certPem: trimmed(certPem))
    }

    private func trimmed(_ value: String) -> String? {
        let text = value.trimmingCharacters(in: .whitespacesAndNewlines)
        return text.isEmpty ? nil : text
    }
}

/// Every call to the chat client, and the state the screens draw.
@MainActor
final class ChatViewModel: ObservableObject {
    @Published var screen: Screen = .connect
    @Published var controlUrl = DevNetwork.defaultControlUrl
    @Published var manual = ManualForm()
    @Published var rooms: [Room] = []
    @Published var openRoomId: Int64?
    @Published var messages: [Message] = [] {
        didSet { rebuildRows() }
    }

    /// The conversation as the screen draws it, rebuilt when the messages change.
    @Published private(set) var rows: [ChatRow] = []
    @Published var unread: Set<Int64> = []
    @Published var username: String?
    @Published var pending = false
    /// Why the last read failed. Cleared by the next read that works.
    @Published var feedError: String?
    /// Why the last thing the reader asked for failed. Held until they ask for something else.
    @Published var actionError: String?
    /// Text a refused send is handing back to the composer. Taken once, then acknowledged.
    @Published var restoredDraft: String?
    @Published var notice: String?
    @Published var target: String?

    private let clocks = Clocks()
    private var client: ChatClient?

    /// The two feeds, stopped whenever the app leaves the foreground.
    private var roomsFeed: Task<Void, Never>?

    /// Restarted on its own when a room is opened, which the room list has no reason to be.
    private var messagesFeed: Task<Void, Never>?
    /// The newest `seq` the reader has seen in each room. Only the room on screen advances it.
    private var seen: [Int64: Int64] = [:]

    var openRoom: Room? { rooms.first { $0.id == openRoomId } }

    // MARK: - Connecting

    /// Moves between the two connect screens, dropping what the other one's attempt reported.
    func show(_ screen: Screen) {
        self.screen = screen
        actionError = nil
    }

    /// Reads the network's description, then connects with it.
    func connect() {
        ask {
            let network = try await DevNetwork.discover(controlUrl: self.controlUrl)
            try await self.open(network.toScionConfig())
        }
    }

    /// Connects with a configuration typed in full.
    func connectManually() {
        ask {
            let config = self.manual.toScionConfig()
            guard !config.endhostApiUrl.isEmpty, !config.baseUrl.isEmpty else {
                throw ChatError.config("an endhost API and a server URL are both needed")
            }
            try await self.open(config)
        }
    }

    /// Builds a client, and proves the server is there.
    private func open(_ config: ScionConfig) async throws {
        let built = ChatClient(transport: try ScionTransport(config: config))
        // Nothing is dialled until a call is made, so the health check is what turns a wrong
        // address into an error on this screen.
        do {
            try await built.health()
        } catch {
            await built.close()
            throw error
        }

        await client?.close()
        client = built
        target = config.target ?? config.baseUrl
        screen = .signIn
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
            self.seed(rooms)

            self.rooms = rooms
            self.username = username
            self.openRoomId = rooms.first?.id
            self.messages = []
            self.screen = .chat
            self.startPolling()
        }
    }

    // MARK: - Rooms and messages

    func openRoom(_ room: Room) {
        guard room.id != openRoomId else { return }
        // Does not advance the read cursor: only a delivered batch does. Marking a room read on
        // opening it would silence a room whose messages never arrived.
        openRoomId = room.id
        messages = []
        unread = unreadRooms(rooms, open: room.id)
        watchMessages()
    }

    /// Creates a room, refusing a name this client will not accept before any call is made.
    func createRoom(_ name: String) {
        if let problem = roomNameProblem(name) {
            actionError = problem
            return
        }

        ask {
            let room = try await self.requireClient().createRoom(name: name)
            self.openRoom(room)
        }
    }

    /// Posts what was typed.
    func send(_ body: String) {
        guard let room = openRoomId, !body.isEmpty else { return }
        // Handed back rather than dropped: the composer has already been cleared, and a refusal the
        // reader cannot see would look like a message that was sent.
        let taken = ask(refused: { _ in self.restoredDraft = body }) {
            try await self.requireClient().send(room: room, body: body)
        }
        if !taken { restoredDraft = body }
    }

    /// Acknowledges ``restoredDraft``, once the composer holds it.
    func draftRestored() {
        restoredDraft = nil
    }

    // MARK: - Polling

    /// Starts both feeds. Called from the screen's lifecycle, so a backgrounded app stops reading.
    func startPolling() {
        watchRooms()
        watchMessages()
    }

    func stopPolling() {
        roomsFeed?.cancel()
        roomsFeed = nil
        messagesFeed?.cancel()
        messagesFeed = nil
    }

    /// Watches the room list.
    private func watchRooms() {
        roomsFeed?.cancel()
        roomsFeed = Task { [weak self] in
            while !Task.isCancelled {
                await self?.readRooms()
                try? await Task.sleep(nanoseconds: roomsInterval)
            }
        }
    }

    /// Watches the open room, from the newest message it holds.
    private func watchMessages() {
        messagesFeed?.cancel()
        guard openRoomId != nil else { return }

        messagesFeed = Task { [weak self] in
            while !Task.isCancelled {
                await self?.readMessages()
                try? await Task.sleep(nanoseconds: messagesInterval)
            }
        }
    }

    private func readRooms() async {
        guard let client else { return }
        do {
            let listing = try await client.rooms()
            let badges = unreadRooms(listing, open: openRoomId)

            if rooms != listing { rooms = listing }
            if unread != badges { unread = badges }
            if feedError != nil { feedError = nil }
        } catch ChatError.sessionExpired {
            signedOut()
        } catch {
            feedError = message(of: error)
        }
    }

    private func readMessages() async {
        guard let client, let open = openRoomId else { return }
        do {
            let batch: [Message]
            if let newest = messages.last?.seq {
                batch = try await client.messagesAfter(room: open, after: newest)
            } else {
                batch = try await client.messagesNewest(room: open)
            }

            // The room is checked, not assumed: a batch in flight when the reader moved on would
            // otherwise land in the room they moved to.
            if !batch.isEmpty, open == openRoomId {
                merge(batch)
                advance(room: open, seq: batch.map(\.seq).max() ?? 0)
                let badges = unreadRooms(rooms, open: open)
                if unread != badges { unread = badges }
            }
            if feedError != nil { feedError = nil }
        } catch ChatError.sessionExpired {
            signedOut()
        } catch {
            feedError = message(of: error)
        }
    }

    /// Marks every room read as it stands, so a session opens quiet.
    private func seed(_ rooms: [Room]) {
        seen = Dictionary(uniqueKeysWithValues: rooms.map { ($0.id, $0.latestSeq) })
    }

    private func advance(room: Int64, seq: Int64) {
        if seq > (seen[room] ?? 0) { seen[room] = seq }
    }

    /// Which rooms hold something unseen. A set: `seq` is server-wide, so a gap counts nothing.
    private func unreadRooms(_ rooms: [Room], open: Int64?) -> Set<Int64> {
        var found: Set<Int64> = []
        for room in rooms where room.id != open && room.latestSeq > (seen[room.id] ?? 0) {
            found.insert(room.id)
        }
        return found
    }

    /// Held messages first, so a drawn row is not replaced.
    private func merge(_ batch: [Message]) {
        var held = messages
        let known = Set(held.map(\.seq))
        held.append(contentsOf: batch.filter { !known.contains($0.seq) })

        let merged = held.sorted { $0.seq < $1.seq }
        if messages != merged { messages = merged }
    }

    private func rebuildRows() {
        rows = chatRows(messages: messages, me: username, clocks: clocks)
    }

    // MARK: - Plumbing

    private func requireClient() throws -> ChatClient {
        guard let client else { throw ChatError.notLoggedIn }
        return client
    }

    private func signedOut() {
        stopPolling()
        username = nil
        messages = []
        unread = []
        screen = .signIn
        actionError = "the session has ended; sign in again"
    }

    private func message(of error: Error) -> String {
        (error as? ChatError)?.errorDescription ?? error.localizedDescription
    }

    /// Runs a call the reader asked for, refusing a second while one is out.
    ///
    /// Answers whether the call was taken, so a caller holding something the reader typed can put
    /// it back.
    @discardableResult
    private func ask(
        refused: @escaping (String) -> Void = { _ in },
        _ work: @escaping () async throws -> Void
    ) -> Bool {
        guard !pending else { return false }
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
        return true
    }
}
