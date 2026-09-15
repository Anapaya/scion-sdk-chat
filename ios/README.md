# SCION Chat for iOS

The chat client as an iOS app: connect, sign in, chat, with every request carried over SCION.

| Path | What it is |
| --- | --- |
| `ChatClient/Sources/ChatClient/ScionTransport.swift` | Builds the SDK client and sends each request. **The only file that mentions SCION.** |
| `ChatClient/Sources/ChatClient/ScionConfig.swift` | What the SDK needs to reach a server. |
| `ChatClient/Sources/ChatClient/ChatClient.swift` | The chat API: register, log in, rooms, messages. |
| `ChatClient/Sources/ChatClient/DevNetwork.swift` | Asks `chat-dev` where it is. A deployed app is told already. |
| `ChatClient/Sources/ChatClient/ChatError.swift` | What a call can fail with. |
| `ChatClient/Sources/ChatClient/Models/` | Generated from the server's OpenAPI document. Not written by hand. |

`App` depends on the `ChatClient` package, and only that package depends on the SDK, so the UI
cannot reach SCION even by accident.

## Requirements

Xcode 16 or newer with an iOS simulator runtime, and
[XcodeGen](https://github.com/yonaskolb/XcodeGen) (`brew install xcodegen`). No Rust and no
cross-compilation: the SCION SDK arrives as a Swift package with a prebuilt XCFramework.

The SCION SDK is on no Swift package registry, so `fetch-sdk.sh` downloads it from its GitHub
release into `ios/libs`, which is gitignored. It checks the archive against the checksum published
beside it, and does nothing when the directory is already there. `sdk.version` is the one place the
version is set.

SwiftPM resolves the package graph before any build phase runs, so this cannot be a step of the
build the way it is on Android. Run it before you generate the project.

## Run it

Start the network on the Mac. The simulator shares the Mac's network, so a loopback address is the
same address inside it:

```bash
cargo run -p chat-dev
```

Then generate the project and run it:

```bash
cd ios
./fetch-sdk.sh
xcodegen generate
open ChatApp.xcodeproj
```

Press **Connect**, register a name, and log in.

`chat-dev` serves its description at `127.0.0.1:8099` over plain HTTP, which App Transport Security
refuses without an exception. `Info.plist` carries `NSAllowsLocalNetworking` for that one call. The
SCION requests are HTTP/3 over the SDK's own UDP sockets, which App Transport Security does not
govern.

## With the Android app or a terminal client at the same time

Nothing to change. The same `chat-dev` serves all of them, and they share rooms. Each client needs
its own token, and every read of `/info` mints one: two clients sharing a token evict each other.

## The generated models

`ChatClient/Sources/ChatClient/Models/` comes from `crates/chat-server/openapi.yaml`, as the Android
client's do. Regenerate them after the API changes:

```bash
npx @openapitools/openapi-generator-cli generate \
  -i crates/chat-server/openapi.yaml \
  -g swift6 \
  --additional-properties=projectName=ChatClient,swiftPackagePath=Sources/ChatClient \
  --global-property models="Room:Message:ServerInfo:Health:LoginRequest:LoginResponse:RegisterRequest:CreateRoomRequest:PostMessageRequest:PostMessageResponse:RoomsResponse:MessagesResponse",supportingFiles=Validation.swift,modelTests=false,modelDocs=false \
  -o ios/ChatClient
```

The model list is explicit because `ErrorCode` is deliberately left out. It is an open enum on the
server — a catch-all variant keeps a client working when a code is added after it ships — and a
generated Swift enum would be closed. `ErrorEnvelope` is hand-written for the same reason, with a
`code: String` that accepts a code this build has never seen.

`Validation.swift` comes with them: the models reference `NumericRule` for the bounds the document
declares.
