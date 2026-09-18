// Copyright 2026 Anapaya Systems

import ChatClient
import SwiftUI

/// How much of the list's width a bubble may take before it wraps.
private let bubbleShare = 0.82
private let corner = 16.0
private let joined = 5.0

/// The rooms, the open room's messages, and the line being typed.
struct ChatScreen: View {
    @ObservedObject var model: ChatViewModel
    @State private var draft = ""
    @State private var showRooms = false
    @State private var naming = false
    @State private var newRoom = ""

    private let clocks = Clocks()

    private var rows: [ChatRow] {
        chatRows(messages: model.messages, me: model.username, clocks: clocks)
    }

    var body: some View {
        VStack(spacing: 0) {
            TopBar(model: model, onMenu: { showRooms = true })
            MessageList(rows: rows)
            if let error = model.feedError { Banner(error) }
            if let error = model.actionError { Banner(error) }
            Composer(draft: $draft, room: model.openRoom?.name, pending: model.pending) {
                model.send(draft)
                draft = ""
            }
        }
        .background(Color.white)
        .onChange(of: model.restoredDraft) {
            guard let text = model.restoredDraft else { return }
            draft = text
            model.draftRestored()
        }
        .sheet(isPresented: $showRooms) {
            RoomList(model: model, onCreate: { naming = true }, onDismiss: { showRooms = false })
        }
        .alert("New room", isPresented: $naming) {
            TextField("Name", text: $newRoom)
            Button("Cancel", role: .cancel) { newRoom = "" }
            Button("Create") {
                model.createRoom(newRoom)
                newRoom = ""
            }
        }
    }
}

private struct TopBar: View {
    @ObservedObject var model: ChatViewModel
    let onMenu: () -> Void

    var body: some View {
        HStack(spacing: 14) {
            Button(action: onMenu) {
                Image(systemName: "line.3.horizontal")
                    .font(.title3)
                    .foregroundStyle(Palette.ink)
                    .overlay(alignment: .topTrailing) {
                        if !model.unread.isEmpty {
                            Circle().fill(Palette.accent).frame(width: 8, height: 8).offset(x: 5, y: -3)
                        }
                    }
            }

            VStack(alignment: .leading, spacing: 1) {
                Text(model.openRoom.map { "#\($0.name)" } ?? "no rooms")
                    .font(.system(size: 19, weight: .bold))
                    .foregroundStyle(Palette.ink)
                if let who = model.username {
                    Text("signed in as \(who)").font(.caption).foregroundStyle(Palette.dim)
                }
            }
            Spacer()

            TransportBadge(live: model.feedError == nil)
        }
        .padding(.horizontal, 16)
        .padding(.top, 8)
        .padding(.bottom, 12)
        .overlay(alignment: .bottom) { Rectangle().fill(Palette.hairline).frame(height: 1) }
    }
}

/**
 That the conversation is carried over SCION, and whether it is still arriving.

 The dot follows the reads, so it reports the link instead of decorating it.
 */
private struct TransportBadge: View {
    let live: Bool

    var body: some View {
        HStack(spacing: 6) {
            Circle().fill(live ? Palette.live : Palette.risk).frame(width: 7, height: 7)
            Text("SCION")
                .font(.system(size: 10.5, weight: .semibold))
                .tracking(1.2)
                .foregroundStyle(Palette.ink)
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 5)
        .overlay(Capsule().stroke(Palette.hairline, lineWidth: 1))
    }
}

private struct MessageList: View {
    let rows: [ChatRow]

    /// Whether the newest message should stay in view. The bottom marker leaving says it should not.
    @State private var following = true

    var body: some View {
        GeometryReader { frame in
            ScrollViewReader { scroll in
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 0) {
                        ForEach(rows) { row in
                            switch row {
                            case .day(let label, _):
                                Text(label)
                                    .font(.system(size: 10.5, weight: .semibold))
                                    .tracking(1)
                                    .foregroundStyle(Palette.dim)
                                    .frame(maxWidth: .infinity)
                                    .padding(.top, 18)
                                    .padding(.bottom, 10)
                            case .said(let said):
                                Bubble(said: said, width: frame.size.width - 28)
                            }
                        }
                        Color.clear
                            .frame(height: 1)
                            .id("newest")
                            .onAppear { following = true }
                            .onDisappear { following = false }
                    }
                    .padding(.horizontal, 14)
                    .padding(.top, 16)
                    .padding(.bottom, 14)
                }
                .onChange(of: rows.count) {
                    guard following else { return }
                    withAnimation { scroll.scrollTo("newest", anchor: .bottom) }
                }
                .onAppear { scroll.scrollTo("newest", anchor: .bottom) }
                .overlay(alignment: .bottom) {
                    if !following {
                        JumpToNewest {
                            following = true
                            withAnimation { scroll.scrollTo("newest", anchor: .bottom) }
                        }
                        .padding(.bottom, 12)
                    }
                }
            }
        }
    }
}

/// The way back to the newest message, shown once the reader has left it.
private struct JumpToNewest: View {
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            HStack(spacing: 6) {
                Text("Newest").font(.system(size: 12.5, weight: .semibold))
                Image(systemName: "arrow.down").font(.system(size: 11, weight: .semibold))
            }
            .foregroundStyle(.white)
            .padding(.horizontal, 15)
            .padding(.vertical, 8)
            .background(Palette.accent, in: Capsule())
        }
        .buttonStyle(.plain)
    }
}

private struct Bubble: View {
    let said: ChatRow.Said
    /// The width of the list, of which a bubble may take ``bubbleShare``.
    let width: CGFloat

    /// A `Spacer` holds this: a maximum width in SwiftUI is one a view grows to fill.
    private var gutter: CGFloat { width * (1 - bubbleShare) }

    var body: some View {
        VStack(alignment: said.mine ? .trailing : .leading, spacing: 0) {
            // Once per run, and never for the reader: their own side names nobody else.
            if said.opensGroup && !said.mine {
                Text(said.message.username)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(nicknameColour(said.message.username))
                    .padding(.top, 12)
                    .padding(.bottom, 2)
                    .padding(.leading, 4)
            }

            HStack(spacing: 0) {
                if said.mine { Spacer(minLength: gutter) }

                // The clock sits beside the text, so a short message stays short.
                HStack(alignment: .lastTextBaseline, spacing: 10) {
                    Text(said.message.body)
                        .font(.system(size: 15))
                        .foregroundStyle(said.mine ? Palette.onOwnBubble : Palette.ink)
                    Text(said.clock)
                        .font(.system(size: 10.5, design: .monospaced))
                        .foregroundStyle(said.mine ? Palette.onOwnBubble : Palette.dim)
                        .fixedSize()
                }
                .padding(.horizontal, 12)
                .padding(.vertical, 8)
                .background(said.mine ? Palette.ownBubble : Palette.fieldFill, in: shape)

                if !said.mine { Spacer(minLength: gutter) }
            }
        }
        .frame(maxWidth: .infinity, alignment: said.mine ? .trailing : .leading)
        .padding(.bottom, said.closesGroup ? (said.nameFollows ? 0 : 10) : 4)
    }

    /// The corner facing a neighbour in the same group flattens, so a run reads as one block.
    private var shape: UnevenRoundedRectangle {
        let big = corner
        let small = joined
        switch (said.mine, said.opensGroup, said.closesGroup) {
        case (true, true, _):
            return .rect(topLeadingRadius: big, bottomLeadingRadius: big, bottomTrailingRadius: small, topTrailingRadius: big)
        case (true, _, true):
            return .rect(topLeadingRadius: big, bottomLeadingRadius: big, bottomTrailingRadius: big, topTrailingRadius: small)
        case (true, _, _):
            return .rect(topLeadingRadius: big, bottomLeadingRadius: big, bottomTrailingRadius: small, topTrailingRadius: small)
        case (false, true, _):
            return .rect(topLeadingRadius: big, bottomLeadingRadius: small, bottomTrailingRadius: big, topTrailingRadius: big)
        case (false, _, true):
            return .rect(topLeadingRadius: small, bottomLeadingRadius: big, bottomTrailingRadius: big, topTrailingRadius: big)
        default:
            return .rect(topLeadingRadius: small, bottomLeadingRadius: small, bottomTrailingRadius: big, topTrailingRadius: big)
        }
    }
}

private struct Composer: View {
    @Binding var draft: String
    let room: String?
    let pending: Bool
    let onSend: () -> Void

    private var ready: Bool {
        !draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty && !pending
    }

    var body: some View {
        HStack(alignment: .bottom, spacing: 10) {
            TextField(room.map { "Message #\($0)" } ?? "Message", text: $draft, axis: .vertical)
                .lineLimit(1...3)
                .font(.system(size: 15))
                .padding(.horizontal, 14)
                .padding(.vertical, 11)
                .background(Palette.fieldFill, in: RoundedRectangle(cornerRadius: 18))
                .overlay(
                    RoundedRectangle(cornerRadius: 18).stroke(Palette.fieldBorder, lineWidth: 1))
                .onSubmit(onSend)

            Button(action: onSend) {
                Image(systemName: "arrow.right")
                    .font(.system(size: 18, weight: .semibold))
                    .foregroundStyle(.white)
                    .frame(width: 42, height: 42)
                    .background(ready ? Palette.accent : Palette.sendDisabled, in: Circle())
            }
            .disabled(!ready)
        }
        .padding(.horizontal, 12)
        .padding(.top, 10)
        .padding(.bottom, 14)
        .overlay(alignment: .top) { Rectangle().fill(Palette.hairline).frame(height: 1) }
    }
}

private struct RoomList: View {
    @ObservedObject var model: ChatViewModel
    let onCreate: () -> Void
    let onDismiss: () -> Void

    var body: some View {
        NavigationStack {
            List(model.rooms) { room in
                Button {
                    model.openRoom(room)
                    onDismiss()
                } label: {
                    HStack {
                        Text("#\(room.name)")
                            .foregroundStyle(room.id == model.openRoomId ? Palette.accent : Palette.ink)
                        Spacer()
                        // A dot, never a number: `seq` is server-wide, so a gap would overstate it.
                        if model.unread.contains(room.id) {
                            Circle().fill(Palette.accent).frame(width: 8, height: 8)
                        }
                    }
                }
            }
            .navigationTitle("Rooms")
            .toolbar {
                Button("New room") {
                    onDismiss()
                    onCreate()
                }
            }
        }
    }
}

private struct Banner: View {
    let text: String

    init(_ text: String) { self.text = text }

    var body: some View {
        Label(text, systemImage: "exclamationmark.triangle")
            .font(.footnote)
            .foregroundStyle(Palette.risk)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 14)
            .padding(.vertical, 8)
            .background(Palette.risk.opacity(0.08))
    }
}
