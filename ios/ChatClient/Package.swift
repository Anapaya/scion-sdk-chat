// swift-tools-version: 6.0
// Copyright 2026 Anapaya Systems

import PackageDescription

let package = Package(
    name: "ChatClient",
    platforms: [.iOS(.v15), .macOS(.v12)],
    products: [
        .library(name: "ChatClient", targets: ["ChatClient"]),
    ],
    dependencies: [
        .package(path: "../libs/scion-http3-swift"),
    ],
    targets: [
        .target(
            name: "ChatClient",
            dependencies: [.product(name: "ScionHTTP3", package: "scion-http3-swift")]
        ),
    ]
)
