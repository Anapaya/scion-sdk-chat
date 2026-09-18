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
    /// A certificate to trust in place of the device's anchors.
    public var certPem: String?

    public init(
        endhostApiUrl: String,
        baseUrl: String,
        snapToken: String? = nil,
        target: String? = nil,
        certPem: String? = nil
    ) {
        self.endhostApiUrl = endhostApiUrl
        self.baseUrl = baseUrl
        self.snapToken = snapToken
        self.target = target
        self.certPem = certPem
    }
}
