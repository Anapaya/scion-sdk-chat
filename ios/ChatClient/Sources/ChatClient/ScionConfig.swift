// Copyright 2026 Anapaya Systems

import Foundation

/// What the SDK needs to reach a server over SCION.
public struct ScionConfig: Sendable, Equatable {
    /// The endhost API of the AS this client attaches to.
    public var endhostApiUrl: String
    /// Where the server is. Its host is the name the certificate is issued for.
    public var baseUrl: String
    /// The token the SNAP underlay authenticates the tunnel with.
    public var snapToken: String?
    /// The server's SCION address, without a port: the port comes from ``baseUrl``.
    public var target: String?
    /// Which certificates this client accepts from the server.
    public var trust: Trust

    public init(
        endhostApiUrl: String,
        baseUrl: String,
        snapToken: String? = nil,
        target: String? = nil,
        trust: Trust = .systemRoots
    ) {
        self.endhostApiUrl = endhostApiUrl
        self.baseUrl = baseUrl
        self.snapToken = snapToken
        self.target = target
        self.trust = trust
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
