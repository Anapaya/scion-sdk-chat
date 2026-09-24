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

            Button { model.show(.manual) } label: {
                Text("Manual SCION configuration")
                    .font(.subheadline.weight(.medium))
                    .foregroundStyle(Palette.accent)
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.plain)
            .disabled(model.pending)

            Progress(pending: model.pending)
            Failure(model.actionError)
            Spacer()
        }
        .padding(24)
    }
}

/// The same configuration, typed out, for a network that describes nothing.
struct ManualScreen: View {
    @ObservedObject var model: ChatViewModel

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text("SCION configuration").font(.largeTitle.bold())

                Field("Endhost API", text: $model.manual.endhostApiUrl)
                Field("Server URL", text: $model.manual.baseUrl)
                Text("Credential").font(.caption).foregroundStyle(Palette.dim)
                Picker("Credential", selection: $model.manual.credential) {
                    ForEach(CredentialChoice.allCases) { choice in
                        Text(choice.rawValue).tag(choice)
                    }
                }
                .pickerStyle(.segmented)

                if model.manual.credential == .apiKey {
                    Field("Auth API key", text: $model.manual.authApiKey)
                } else {
                    Field("SNAP token", text: $model.manual.snapToken)
                }

                Field("Target - the server's SCION address", text: $model.manual.target)

                Text("Trust").font(.caption).foregroundStyle(Palette.dim)
                Picker("Trust", selection: $model.manual.trust) {
                    ForEach(TrustChoice.allCases) { choice in
                        Text(choice.rawValue).tag(choice)
                    }
                }
                .pickerStyle(.segmented)

                if model.manual.trust == .pinned {
                    Field("Certificate (PEM)", text: $model.manual.certPem, lines: 4)
                }

                Button(action: model.connectManually) {
                    Text("Connect").frame(maxWidth: .infinity)
                }
                .buttonStyle(PrimaryButton())
                .disabled(model.pending)

                Button { model.show(.connect) } label: {
                    Text("Read it from a development network")
                        .font(.subheadline.weight(.medium))
                        .foregroundStyle(Palette.accent)
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.plain)
                .disabled(model.pending)

                Progress(pending: model.pending)
                Failure(model.actionError)
            }
            .padding(24)
        }
        .textInputAutocapitalization(.never)
        .autocorrectionDisabled()
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
    /// How far the field grows before it scrolls instead.
    var lines = 1

    init(_ label: String, text: Binding<String>, secure: Bool = false, lines: Int = 1) {
        self.label = label
        self._text = text
        self.secure = secure
        self.lines = lines
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(label).font(.caption).foregroundStyle(Palette.dim)
            Group {
                if secure {
                    SecureField("", text: $text)
                } else {
                    TextField("", text: $text, axis: .vertical).lineLimit(1...lines)
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
