// Copyright 2026 Anapaya Systems

import Foundation

/// What the SDK needs to reach a server over SCION.
public struct ScionConfig: Sendable, Equatable {
    /// The endhost API of the AS this client attaches to.
    public var endhostApiUrl: String
    /// Where the server is. Its host is the name the certificate is issued for.
    public var baseUrl: String
    /// How the client proves it may use the SNAP.
    public var credential: Credential
    /// The server's SCION address, without a port: the port comes from ``baseUrl``.
    public var target: String?
    /// Which certificates this client accepts from the server.
    public var trust: Trust

    public init(
        endhostApiUrl: String,
        baseUrl: String,
        credential: Credential = .none,
        target: String? = nil,
        trust: Trust = .systemRoots
    ) {
        self.endhostApiUrl = endhostApiUrl
        self.baseUrl = baseUrl
        self.credential = credential
        self.target = target
        self.trust = trust
    }
}

/// The authority that mints tokens for Anapaya's own network.
public let anapayaAa = "https://auth.scion.anapaya.net"

/// How a client proves to the SNAP that it may use the network.
///
/// An API key is the long-lived secret. The authority mints tokens from it, each good for a day at
/// most, and the client renews them for as long as it runs.
public enum Credential: Sendable, Equatable {
    /// Nothing to prove. An endhost API on an appliance asks for no token.
    case none
    /// One token, already minted. This is what `chat-dev` hands out.
    case token(String)
    /// A key the client exchanges for tokens, and keeps exchanging.
    case apiKey(ApiKeyAuth)
}

/// A key, and where to spend it.
public struct ApiKeyAuth: Sendable, Equatable {
    /// The key itself.
    public var key: String
    /// The authority that mints tokens for it.
    public var aaUrl: String
    /// What the client calls itself in the authority's records.
    public var deviceId: String

    public init(key: String, aaUrl: String = anapayaAa, deviceId: String = "chat-ios") {
        self.key = key
        self.aaUrl = aaUrl
        self.deviceId = deviceId
    }
}

/// Which certificates a client accepts from the server.
public enum Trust: Sendable, Equatable {
    /// The anchors the device ships.
    case systemRoots
    /// One certificate, in place of the device's anchors. A self-signed server needs this.
    case pinned(String)
    /// Accept any certificate.
    ///
    /// Every reply can come from anyone on the path. It exists so a demo can run against a
    /// self-signed server without moving its certificate to the device first.
    case insecure
}
