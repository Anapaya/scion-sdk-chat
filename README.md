# scion-sdk-chat

Chat demo application using scion-sdk.

The workspace holds five crates:

- `chat-core`, the API's request and response types
- `chat-server`, the server
- `chat-client-core`, chat client functionality as library
- `chat-ui-ratatui`, a terminal UI on top of it
- `chat-dev`, a SCION network on this machine with the server in it

`cargo doc_dx --open` renders them all.

## chat-server guide

`--transport tcp` serves the API as plain HTTP, with no TLS and no SCION. It is a development mode.

```sh
cargo run -p chat-server -- --transport tcp --listen 127.0.0.1:8080 --data-dir ./data
```

`cargo run -p chat-server -- --help` lists every flag with its default and its `CHAT_*` environment
fallback.

### Endpoints

Every route sits under `/api/v1`, and every route except `/healthz`, `/server`, `/register` and
`/login` needs `A="authorization: Bearer $TOKEN"`, from `login`.

The server describes itself at `/.well-known/openapi.json`. The same document is committed as YAML,
which diffs better, at [`crates/chat-server/openapi.yaml`](crates/chat-server/openapi.yaml), so the
API surface is readable, and reviewable, without running anything. A test compares the two; it
rewrites the file when run as:

```sh
CHAT_UPDATE_OPENAPI=1 cargo test -p chat-server
```

## chat-client-core guide

The typed API, the session, and the underlying transport:

- `ScionTransport` carries HTTP/3 over SCION, against the server's `--transport scion` mode
- `TcpTransport` speaks plain HTTP to the server's `--transport tcp` mode
- `MockTransport` answers from a script instead of a network, which is how a test produces what a
  real server cannot produce on demand

**`--transport` picks the transport.** Each transport is served under exactly one scheme —
`scion` under `https`, `tcp` under `http` — and the URL is checked against the choice. SCION also needs an endhost API, which is how it reaches the network at all; the rest is optional.


## chat-ui-ratatui guide

Three screens over `chat-client-core` — connect, sign in, chat. Against a server in
`--transport tcp` mode:

```sh
cargo run -p chat-server -- --transport tcp --listen 127.0.0.1:8080 --data-dir ./data
cargo run -p chat-ui-ratatui -- --transport tcp
```

Every field of the connect screen also has a flag, so a launch can arrive with the form answered:

| flag | environment | what it is |
| --- | --- | --- |
| `--transport` | `CHAT_CLIENT_TRANSPORT` | `scion` or `tcp`. Defaults to `scion` |
| `--server-url` | `CHAT_CLIENT_SERVER_URL` | where the server is |
| `--endhost-api` | `CHAT_CLIENT_ENDHOST_API` | how the client finds SCION. Required by `--transport scion` |
| `--target` | `CHAT_CLIENT_TARGET` | the server's SCION address, for a host with no TSAR record |
| `--cert-path` | `CHAT_CLIENT_CERT_PATH` | a certificate to trust instead of the system roots |
| `--snap-token` | `CHAT_CLIENT_SNAP_TOKEN` | the token the SNAP underlay asks for |

The client reads `CHAT_CLIENT_*` and the server reads `CHAT_*`. They must not be merged: the server
sits in one AS and the client attaches to another, so a shared `CHAT_ENDHOST_API` would point the
client at the wrong endhost API.

The screens draw and read keys; `app.rs` holds every call to the client, so there is one place to
look for how the SDK is used.

## chat-dev

Three autonomous systems in a star, with the chat server in the middle one. The network lives for
exactly as long as this process does. The underlay is SNAP: over SNAP an endpoint is addressed at
the address its tunnel observed, which is what a client behind a translation needs.

### The topology

```text
  ┌────────────────────────┐
  │ 1-ff00:0:132           │
  │ the local AS           │──┐ iface 1 ↔ 3
  │ published at 127.0.0.1 │  │
  └────────────────────────┘  │      ┌──────────────────────────────┐
                              └──────│ 2-ff00:0:212                 │
                                     │ the server AS                │
                              ┌──────│ the chat server listens here │
  ┌────────────────────────┐  │      └──────────────────────────────┘
  │ 2-ff00:0:222           │  │
  │ the emulator AS        │──┘ iface 2 ↔ 4
  │ published at 10.0.2.2  │
  └────────────────────────┘
```

| AS | who attaches to it | published at |
| --- | --- | --- |
| `1-ff00:0:132` | clients on this machine | `127.0.0.1` |
| `2-ff00:0:212` | the chat server | `127.0.0.1` |
| `2-ff00:0:222` | a client on an Android emulator | `10.0.2.2` |

A client AS each, because an address is published **per AS**. Two clients in one AS are told the
same address, and an emulator reaches this host at `10.0.2.2` while a local client reaches it at
`127.0.0.1`. A star, so each client AS is linked to the server's and to nothing else.

Where this is built:

| file | what it decides |
| --- | --- |
| [`crates/chat-dev/src/topology.rs`](crates/chat-dev/src/topology.rs) | the three ASes, the two links, and the SNAP endpoint and endhost API in each |
| [`crates/chat-dev/src/lib.rs`](crates/chat-dev/src/lib.rs) | the address published to each AS, and which description `GET /info` answers with |

### Starting it

```sh
cargo run -p chat-dev
```

One command starts the whole network **and** the chat server inside this process: the three ASes,
an endhost API and a SNAP endpoint for each, the control API on port 8099, and the server on port
8443. Ctrl+C stops all of it together.

Almost nothing about the network can be written down in advance: the endhost APIs take whatever
ports are free, the token is minted per run, and the certificate is generated. So the network
describes itself. One line of JSON goes to standard output at startup, and the same document is
served at `GET /info` on `--control-port` (8099) over **plain TCP**, so a client that failed to
connect can still read it.

```sh
curl -s http://127.0.0.1:8099/info | jq
```

```json
{
  "control_url": "http://127.0.0.1:8099",
  "underlay": "snap",
  "server": "embedded",
  "endhost_api_url": "http://127.0.0.1:65263/",
  "server_endhost_api_url": "http://127.0.0.1:65264/",
  "client_isd_as": "1-ff00:0:132",
  "auth_token": "eyJ0eXAiOiJKV1Qi…",
  "auth_token_file": "/tmp/.tmpTFIcwA/snap.token",
  "base_url": "https://localhost:8443",
  "target": "2-ff00:0:212,127.0.0.1",
  "ca_pem": "-----BEGIN CERTIFICATE-----\n…",
  "ca_path": "/tmp/.tmpTFIcwA/cert.pem",
  "ca_fingerprint": "a4df3e47…",
  "data_dir": "/tmp/.tmpTFIcwA",
  "chat_server_args": ["--transport", "scion", "--listen", "127.0.0.1:8443", "…"]
}
```

Field by field:

| field | what a client does with it |
| --- | --- |
| `base_url` | the server's URL. `https`, which is the scheme `--transport scion` is served under |
| `endhost_api_url` | the endhost API of the **client's** AS. How the client reaches SCION at all |
| `server_endhost_api_url` | the endhost API of the **server's** AS, for a server started separately |
| `client_isd_as` | the AS this description attaches a client to |
| `target` | the server's SCION address. This topology has no TSAR records, so the address is given |
| `auth_token` | the SNAP token, minted fresh on every read and belonging to one client |
| `auth_token_file` | the server's own token, on disk. A separate one |
| `ca_pem` / `ca_path` | the certificate to trust, inline and on disk |
| `chat_server_args` | the exact arguments that join a separately started server to this network |

Each endhost API belongs to its own AS, so a client uses `endhost_api_url` and a server uses
`server_endhost_api_url`. `ca_pem` is inline as well as on disk, for a client that reads a
different filesystem.

`GET /info` answers with the description that matches the address it was asked at. A request to
`127.0.0.1` is given the local AS, and one to `10.0.2.2` is given the emulator AS. Both name the
same server, the same certificate and the same SCION address; only the way in differs.

Standard error carries the logs and the same description in the shape a person reads. Ctrl+C stops
everything, and so does closing standard input, which is how a harness stops it.

### Connecting a client

Every field above has a flag and an environment variable, so the whole description can go into the
environment in one step. Read it once per terminal, because each client needs a token of its own:

```sh
eval "$(curl -s http://127.0.0.1:8099/info | jq -r '
  "export CHAT_CLIENT_SERVER_URL=\(.base_url | @sh)",
  "export CHAT_CLIENT_ENDHOST_API=\(.endhost_api_url | @sh)",
  "export CHAT_CLIENT_TARGET=\(.target | @sh)",
  "export CHAT_CLIENT_CERT_PATH=\(.ca_path | @sh)",
  "export CHAT_CLIENT_SNAP_TOKEN=\(.auth_token | @sh)"
')"

cargo run -p chat-ui-ratatui -- --transport scion
```

Repeat both commands in a third terminal for a second user. Their messages cross from
`1-ff00:0:132` to `2-ff00:0:212`.

Each client needs its own token. The control plane keeps one tunnel per subscriber, so a second
client on the same token evicts the first, and the first then stops working silently while the
server logs `wireguard error on incoming packet`. Running the `eval` again in each terminal is what
keeps them apart.

### Running the server yourself

`--no-server` holds up only the network and leaves the server out of it:

```sh
cargo run -p chat-dev -- --no-server
```

`cargo run -p chat-server` then starts the chat server as a process of its own, against the network
already running. It is the same server `chat-dev` would have started, and it can now be stopped,
rebuilt and restarted while the network stays up, which is what makes it worth the second terminal
while working on the server. `chat_server_args` is the command that joins it:

```sh
cargo run -p chat-server -- $(curl -s http://127.0.0.1:8099/info | jq -r '.chat_server_args | join(" ")')
```

Those arguments carry the server's own endhost API, its own token file, the listen address and the
data directory. The data directory is shared on purpose, so the server presents the certificate the
description names.

### From another machine

Everything above binds to `127.0.0.1`, which is reachable from this machine alone. `--bind-ip`
moves every part of the network — the control API, all three endhost APIs, the SNAP endpoints and
the server — to one address another machine can reach:

```sh
cargo run -p chat-dev -- --bind-ip 192.168.1.20
```

It must be a real address on this machine. A wildcard is refused: the SNAP tunnel is dialled at the
bound address, and `0.0.0.0` names no host, so the network fails to start with `error establishing
SNAP tunnel`.

Every client then uses that one address, this machine included, so the name `localhost` is what is
given up in exchange. The address is a real one on a real network, so it changes when DHCP moves
you and disappears when you work offline.

### From an Android emulator

Nothing to pass:

```sh
cargo run -p chat-dev
```

An emulator reaches the host's loopback as `10.0.2.2`, and `2-ff00:0:222` is published at exactly
that address. A client there asks the same `GET /info`, arrives at `10.0.2.2`, and is given the
emulator's description; a terminal client on this machine asks at `127.0.0.1` and is given its own.
Both join the same rooms. `--emulator-ip` moves that address.

The AS is ready and the network serves it. The Android client that attaches to it is a later
change.

## Development

The pinned toolchain in `rust-toolchain.toml` is picked up automatically by rustup. CI runs the
checks the cargo aliases in `.cargo/config.toml` define, on Linux and Windows:

```sh
cargo clippy_ci
cargo test_ci
cargo doc_ci
```

Formatting runs on a pinned nightly, because `rustfmt.toml` uses nightly-only options:

```sh
cargo +nightly-2026-03-12 fmt --all
```

### The database

SQLite is compiled into the binary, so there is no database service to install, start, or connect
to. The database is a single file; the store creates it, and the directory holding it, when
absent. Deleting it starts over.

### `.sqlx` purpose

The server's SQL is checked against the schema while it compiles, so the compiler needs to know
the schema. `crates/chat-server/.sqlx/` is what it was told, one JSON file per query.

It is committed so that **compiling** needs no database and no tooling: `cargo build`, `cargo test`
and `cargo run` all work on a fresh clone without `DATABASE_URL` or `sqlx-cli`. Without it the
crate does not compile.

That the *server* needs no database service is separate — SQLite and `schema.sql` are compiled
into the binary, which is true with or without this folder.

You only need `sqlx-cli` to add or change a query, which requires regenerating it:

```sh
cargo install --version 0.9.0 sqlx-cli --no-default-features --features sqlite  # match Cargo.toml
cd crates/chat-server
DB=$(mktemp -d)/prepare.db   # a fresh file: CREATE TABLE IF NOT EXISTS would leave a stale one stale
sqlite3 "$DB" < src/store/sqlite/schema.sql
DATABASE_URL="sqlite://$DB" cargo sqlx prepare -- --all-targets
```

Forgetting to is a compile error. Metadata that has drifted from the schema is worse — it still
compiles — so CI runs the same commands with `--check`.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
