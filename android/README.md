# SCION Chat for Android

The chat client as an Android app: connect, sign in, chat, with every request carried over SCION.

| File | What it is |
| --- | --- |
| `chat-client/ScionTransport.kt` | Builds the SDK client and sends each request. **The only file that mentions SCION.** |
| `chat-client/ChatClient.kt` | The chat API: register, log in, rooms, messages. |
| `chat-client/Feeds.kt` | Polling, as flows. |
| `chat-client/DevNetwork.kt` | Asks `chat-dev` where it is. A deployed app knows this already. |
| `chat-client/ChatError.kt` | What a call can fail with. |
| `chat-client/model/` | Generated from the server's OpenAPI document. Not written by hand. |
| `app/ChatViewModel.kt` | Every call to the client, and the state the screens draw. **The only file that talks to a server.** |
| `app/RoomName.kt` | What a room may be called, and which rooms hold something unread. |
| `app/ui/chat/` | The chat screen: the room drawer, the messages, the composer. |
| `app/ui/theme/` | The colour scheme, and the nickname colours the terminal client also uses. |
| `app/ui/Screens.kt` | Connecting and signing in. |

`app` depends on `chat-client`, and only `chat-client` depends on the SDK, so the UI cannot reach
SCION even by accident. Nothing under `ui/` calls a client: a screen draws what it is given and
reports what was tapped.

The room list is a drawer on a phone and stays open from 840dp, so a tablet or an unfolded foldable
reads like the terminal client's sidebar. A room holding unread messages carries a dot, never a
count — `seq` is assigned server-wide, so the gap between two of them spans other rooms.

## Requirements

JDK 17, an Android SDK with `platforms;android-35` and `build-tools;35.0.0`, and a running
emulator. No NDK and no Rust cross-compilation: the SDK arrives as a published AAR.

Fetch it once, from the repository root:

```bash
gh release download v0.8.0 --repo Anapaya/scion-sdk -p 'scion-http3-android-*-maven.zip'
unzip -q scion-http3-android-0.8.0-maven.zip -d android/libs/maven
```

It is not on any public repository, so Gradle reads it from `android/libs/maven`, which is
gitignored. The build says this again if the directory is missing.

## Run it

Start the network on the host. The emulator reaches the host's loopback as `10.0.2.2`, which is
what the app is pre-filled with:

```bash
cargo run -p chat-dev
```

No flag: the network has an AS for the emulator, published at `10.0.2.2`, and answers `GET /info`
with that AS's description when the request arrives at that address.

`10.0.2.2` is not a route. The emulator runs a user-mode network stack that terminates the app's
connection and opens a new one from the host process, and `10.0.2.2` is the address it special-cases
to the host's **loopback**. So it reaches only what is bound there, and it exists nowhere outside the
emulator.

Then install the app:

```bash
cd android && ./gradlew :app:installDebug
```

Press **Connect**, register a name, and log in. Anything typed goes to the open room, and **New
room** in the drawer makes another. The terminal client's `/room <name>` is not repeated here: a
phone has room for a button, so every line typed is a message.

## With a terminal client at the same time

Nothing to change. The same `chat-dev` serves both, and the two clients share rooms:

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

The terminal client gets the default AS and the emulator gets the fallback one. The root
[README](../README.md#the-topology) describes why, and which file sets it up.

Each client needs its own token, and every read of `/info` mints one. Two clients sharing a token
evict each other, and it is the first that stops working.

## From another machine

A phone on the same network is neither of the two cases above: it reaches this host at its LAN
address, and nothing special-cases that. Move the whole network there:

```bash
cargo run -p chat-dev -- --bind-ip 192.168.1.20      # this host's address on the network
```

Then type that address into the app's Control URL. The emulator's AS is still published at
`10.0.2.2`, which a real device cannot reach, so pass `--emulator-ip 192.168.1.20` as well if an
emulator has to join that run too.

## The generated models

`chat-client/model/` comes from `crates/chat-server/openapi.yaml`. Regenerate it after the API
changes:

```bash
npx @openapitools/openapi-generator-cli generate \
  -i crates/chat-server/openapi.yaml \
  -g kotlin \
  --additional-properties=serializationLibrary=kotlinx_serialization,modelPackage=com.anapaya.chat.client.model \
  --global-property models="Room:Message:ServerInfo:Health:LoginRequest:LoginResponse:RegisterRequest:CreateRoomRequest:PostMessageRequest:PostMessageResponse:RoomsResponse:MessagesResponse",modelTests=false,modelDocs=false \
  -o android/chat-client
```

The model list is explicit because `ErrorCode` is deliberately left out. It is an open enum on the
server — a catch-all variant keeps a client working when a code is added after it ships — and a
generated Kotlin enum would be closed. `ChatError` is hand-written for the same reason, with an
`Api(code: String)` that accepts a code this build has never seen.
