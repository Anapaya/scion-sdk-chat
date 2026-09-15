// Copyright 2026 Anapaya Systems

import SwiftUI

@main
struct ChatApp: App {
    @StateObject private var model = ChatViewModel()
    @Environment(\.scenePhase) private var phase

    var body: some Scene {
        WindowGroup {
            Group {
                switch model.screen {
                case .connect: ConnectScreen(model: model)
                case .manual: ManualScreen(model: model)
                case .signIn: SignInScreen(model: model)
                case .chat: ChatScreen(model: model)
                }
            }
            // Polling runs only while the app is in front, so a backgrounded app stops asking for
            // messages nobody is reading.
            .onChange(of: phase) {
                guard model.screen == .chat else { return }
                if phase == .active { model.startPolling() } else { model.stopPolling() }
            }
            .alert(
                model.notice ?? "",
                isPresented: Binding(
                    get: { model.notice != nil },
                    set: { shown in if !shown { model.notice = nil } })
            ) {
                Button("OK", role: .cancel) {}
            }
        }
    }
}
