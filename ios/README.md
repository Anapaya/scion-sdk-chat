# SCION Chat for iOS

The chat client as an iOS app: connect, sign in, chat, with every request carried over SCION.

| File | What it is |
| --- | --- |
| `ChatClient/ScionTransport.swift` | Builds the SDK client and sends each request. **The only file that mentions SCION.** |
| `ChatClient/ScionConfig.swift` | What the SDK needs to reach a server. |
| `ChatClient/ChatClient.swift` | The chat API: register, log in, rooms, messages. |
| `ChatClient/DevNetwork.swift` | Asks `chat-dev` for a `ScionConfig`. A deployed app is told one. |
| `ChatClient/ChatError.swift` | What a call can fail with. |
| `ChatClient/Models/` | Generated from the server's OpenAPI document. Not written by hand. |
| `App/ChatViewModel.swift` | Every call to the client, and the state the screens draw. **The only file that talks to a server.** |
| `App/ChatRows.swift` | What a room may be called, and which rooms hold something unread. |
| `App/ChatScreen.swift` | The chat screen: the room list, the messages, the composer. |
| `App/Theme.swift` | The colour scheme, and the nickname colours the terminal client also uses. |
| `App/ConnectScreens.swift` | Connecting and signing in. |

The paths under `ChatClient/` are inside `ChatClient/Sources/ChatClient/`.

The SDK is a dependency of the `ChatClient` package alone. `App` depends on `ChatClient`, so the UI
cannot reach SCION by accident. No view holds a client.

Two screens fill a `ScionConfig`: one reads the description a development network serves, the other
takes the same fields by hand.

## Requirements

Xcode 16 or newer with an iOS simulator runtime, and
[XcodeGen](https://github.com/yonaskolb/XcodeGen) (`brew install xcodegen`). No NDK and no Rust
cross-compilation: the SCION SDK arrives as a Swift package with a prebuilt XCFramework.

The SCION SDK is on no Swift package registry, so `fetch-sdk.sh` downloads it from its GitHub
release into `ios/libs`, which is gitignored. The version comes from `sdk.version`, and the script
checks the archive against the checksum published beside it.

SwiftPM resolves the package graph before any build phase runs, so this cannot be a step of the
build the way it is on Android. Run it before you generate the project.

## Run it

Start the network on the Mac. The simulator shares the Mac's network, so a loopback address is the
same address inside it:

```bash
cargo run -p chat-dev
```

No flag, and no AS of its own: the address the simulator reaches the server at is the address
outside it, which is what the emulator on Android cannot do.

Then generate the project and run it:

```bash
cd ios
./fetch-sdk.sh
xcodegen generate
open ChatApp.xcodeproj
```

`chat-dev` serves its description over plain HTTP, which App Transport Security refuses without an
exception. `Info.plist` carries `NSAllowsLocalNetworking` for that one call. The chat requests are
HTTP/3 over the SDK's own UDP sockets, which App Transport Security does not govern.

## With other clients at the same time

Nothing to change. The same `chat-dev` serves the Android app and the terminal client too, and they
all share rooms:

```bash
eval "$(curl -s http://127.0.0.1:8099/info | jq -r '
  "export CHAT_CLIENT_SERVER_URL=\(.base_url | @sh)",
  "export CHAT_CLIENT_ENDHOST_API=\(.endhost_api_url | @sh)",
  "export CHAT_CLIENT_TARGET=\(.target | @sh)",
  "export CHAT_CLIENT_CERT_PATH=\(.ca_path | @sh)",
  "export CHAT_CLIENT_SNAP_TOKEN=\(.auth_token | @sh)"
')"

cargo run -p chat-ui-ratatui
```

The simulator and the terminal client both get the default AS, because both reach the server at the
same address. The root [README](../README.md#the-topology) describes the topology, and which file
sets it up.

Each client needs its own token, and every read of `/info` mints one. Two clients sharing a token
evict each other, and it is the first that stops working.

## From another machine

An iPhone on the same network reaches this host at its LAN address. Move the whole network there:

```bash
cargo run -p chat-dev -- --bind-ip 192.168.1.20      # this host's address on the network
```

Then type that address into the app's Control URL.

## The generated models

`ChatClient/Sources/ChatClient/Models/` comes from `crates/chat-server/openapi.yaml`, as the Android
client's models do. Regenerate them after the API changes, from the repository root:

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
