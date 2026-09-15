// Copyright 2026 Anapaya Systems

import SwiftUI

/// Where a network that describes itself is asked for that description.
struct ConnectScreen: View {
    @ObservedObject var model: ChatViewModel

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Connect").font(.largeTitle.bold())
            Text("The address a development network serves its description at. Everything after "
                + "this goes over SCION.")
                .font(.subheadline)
                .foregroundStyle(Palette.dim)

            Field("Control URL", text: $model.controlUrl)
                .keyboardType(.URL)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()

            Button(action: model.connect) {
                Text("Connect").frame(maxWidth: .infinity)
            }
            .buttonStyle(PrimaryButton())
            .disabled(model.pending)

            Progress(pending: model.pending)
            Failure(model.actionError)
            Spacer()
        }
        .padding(24)
    }
}

/// Registering and logging in are separate, as the API keeps them.
struct SignInScreen: View {
    @ObservedObject var model: ChatViewModel
    @State private var username = ""
    @State private var password = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Sign in").font(.largeTitle.bold())
            if let target = model.target {
                Text("over SCION, to \(target)")
                    .font(.footnote)
                    .foregroundStyle(Palette.dim)
            }

            Field("Username", text: $username)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
            Field("Password", text: $password, secure: true)

            HStack(spacing: 12) {
                Button { model.register(username: username, password: password) } label: {
                    Text("Register").frame(maxWidth: .infinity)
                }
                .buttonStyle(SecondaryButton())

                Button { model.logIn(username: username, password: password) } label: {
                    Text("Log in").frame(maxWidth: .infinity)
                }
                .buttonStyle(PrimaryButton())
            }
            .disabled(model.pending)

            Progress(pending: model.pending)
            Failure(model.actionError)
            Spacer()
        }
        .padding(24)
    }
}

// MARK: - Pieces both screens use

struct Field: View {
    let label: String
    @Binding var text: String
    var secure = false

    init(_ label: String, text: Binding<String>, secure: Bool = false) {
        self.label = label
        self._text = text
        self.secure = secure
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label).font(.caption).foregroundStyle(Palette.dim)
            Group {
                if secure {
                    SecureField("", text: $text)
                } else {
                    TextField("", text: $text)
                }
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 11)
            .background(Palette.fieldFill, in: RoundedRectangle(cornerRadius: 12))
            .overlay(
                RoundedRectangle(cornerRadius: 12).stroke(Palette.fieldBorder, lineWidth: 1))
        }
    }
}

struct PrimaryButton: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.body.weight(.semibold))
            .foregroundStyle(.white)
            .padding(.vertical, 13)
            .background(
                configuration.isPressed ? Palette.strongBlue : Palette.accent,
                in: Capsule())
    }
}

struct SecondaryButton: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.body.weight(.semibold))
            .foregroundStyle(Palette.accent)
            .padding(.vertical, 13)
            .background(
                configuration.isPressed ? Palette.fieldFill : Color.clear, in: Capsule())
            .overlay(Capsule().stroke(Palette.fieldBorder, lineWidth: 1))
    }
}

struct Progress: View {
    let pending: Bool

    var body: some View {
        if pending {
            ProgressView().tint(Palette.accent)
        }
    }
}

struct Failure: View {
    let text: String?

    init(_ text: String?) { self.text = text }

    var body: some View {
        if let text {
            Label(text, systemImage: "exclamationmark.triangle")
                .font(.footnote)
                .foregroundStyle(Palette.risk)
        }
    }
}
