# SCION Chat for iOS

The chat client as an iOS app: connect, sign in, chat, with every request carried over SCION.

| Path | What it is |
| --- | --- |
| `ChatClient/Sources/ChatClient/ScionTransport.swift` | Builds the SDK client and sends each request. **The only file that mentions SCION.** |
| `ChatClient/Sources/ChatClient/ScionConfig.swift` | What the SDK needs to reach a server. |
| `ChatClient/Sources/ChatClient/ChatClient.swift` | The chat API: register, log in, rooms, messages. |
| `ChatClient/Sources/ChatClient/DevNetwork.swift` | Asks `chat-dev` where it is. A deployed app is told already. |
| `ChatClient/Sources/ChatClient/ChatError.swift` | What a call can fail with. |
| `ChatClient/Sources/ChatClient/Models.swift` | The API's types, from the server's OpenAPI document. |

`App` depends on the `ChatClient` package, and only that package depends on the SDK, so the UI
cannot reach SCION even by accident.

## Requirements

Xcode 16 or newer with an iOS simulator runtime, and
[XcodeGen](https://github.com/yonaskolb/XcodeGen) (`brew install xcodegen`). No Rust and no
cross-compilation: the SDK arrives as a Swift package with a prebuilt XCFramework.

Fetch it once, from the repository root:

```bash
gh release download v0.8.0 --repo Anapaya/scion-sdk -p 'scion-http3-swift-0.8.0.zip'
unzip -q scion-http3-swift-0.8.0.zip -d ios/libs
```

`ios/libs` is gitignored. SwiftPM downloads the XCFramework named in the package's `Package.swift`
on the first build.

## Run it

Start the network on the Mac. The simulator shares the Mac's network, so a loopback address is the
same address inside it:

```bash
cargo run -p chat-dev
```

Then generate the project and run it:

```bash
cd ios && xcodegen generate && open ChatApp.xcodeproj
```

Press **Connect**, register a name, and log in.

`chat-dev` serves its description at `127.0.0.1:8099` over plain HTTP, which App Transport Security
refuses without an exception. `Info.plist` carries `NSAllowsLocalNetworking` for that one call. The
SCION requests are HTTP/3 over the SDK's own UDP sockets, which App Transport Security does not
govern.

## With the Android app or a terminal client at the same time

Nothing to change. The same `chat-dev` serves all of them, and they share rooms. Each client needs
its own token, and every read of `/info` mints one: two clients sharing a token evict each other.
