# scion-sdk-chat

A chat application that shows how to use the SCION SDK: a server, a client library, a terminal UI,
and a SCION network to run them in.

## How to use it

### Over SCION

`chat-dev` starts a SCION network on this machine with the chat server in it, and describes itself
on standard output:

```sh
cargo run -p chat-dev
```

It prints a command line for the terminal UI. Paste it into a second terminal:

```sh
cargo run -p chat-ui-ratatui -- \
  --server-url https://localhost:8443 \
  --endhost-api http://127.0.0.1:65263/ \
  --target 2-ff00:0:212,127.0.0.1 \
  --cert-path /tmp/.tmpTFIcwA/cert.pem \
  --snap-token "$(curl -s http://127.0.0.1:8099/info | jq -r .auth_token)"
```

Paste it into a third terminal for a second user. Their messages cross from `1-ff00:0:132` to
`2-ff00:0:212`.

**Each client needs its own token**, which is why the command fetches one rather than carrying one.
A token names a SNAP subscriber, and the control plane keeps one tunnel per subscriber, so a second
client on the same token evicts the first — and it is the first that stops working, silently, while
the server logs `wireguard error on incoming packet`. Every read of `/info` mints a fresh one.

Or read the whole form into the environment, which is less to paste per terminal:

```sh
eval "$(curl -s http://127.0.0.1:8099/info | jq -r '
  "export CHAT_CLIENT_SERVER_URL=\(.base_url | @sh)",
  "export CHAT_CLIENT_ENDHOST_API=\(.endhost_api_url | @sh)",
  "export CHAT_CLIENT_TARGET=\(.target | @sh)",
  "export CHAT_CLIENT_CERT_PATH=\(.ca_path | @sh)",
  "export CHAT_CLIENT_SNAP_TOKEN=\(.auth_token | @sh)"
')"

cargo run -p chat-ui-ratatui
```

The endhost API, the token and the certificate change every run, so read them from `chat-dev`
rather than writing them down. Ctrl+C stops the network, and everything in it.

### Over plain TCP

No TLS and no SCION, for work on the UI itself:

```sh
cargo run -p chat-server -- --transport tcp --listen 127.0.0.1:8080 --data-dir ./data
cargo run -p chat-ui-ratatui
```

### Options

**The URL scheme picks the client's transport**: `http` plain, `https` over SCION. Every field of
the connect screen also has a flag, so a launch can arrive with the form answered:

| flag | environment | what it is |
| --- | --- | --- |
| `--server-url` | `CHAT_CLIENT_SERVER_URL` | where the server is, and which transport to use |
| `--endhost-api` | `CHAT_CLIENT_ENDHOST_API` | how the client finds SCION. Required by `https` |
| `--target` | `CHAT_CLIENT_TARGET` | the server's SCION address, for a host with no TSAR record |
| `--cert-path` | `CHAT_CLIENT_CERT_PATH` | a certificate to trust instead of the system roots |
| `--snap-token` | `CHAT_CLIENT_SNAP_TOKEN` | the token the SNAP underlay asks for |

Only `--endhost-api` is required by `https`; the rest may be left blank and typed on the screen.

The client reads `CHAT_CLIENT_*` and the server reads `CHAT_*`. They must not be merged: the server
sits in one AS and the client attaches to another, so a shared `CHAT_ENDHOST_API` would point the
client at the wrong endhost API.

`--help` on any of the three binaries lists every flag with its default.

## Repo setup

Five crates:

| crate | what it is |
| --- | --- |
| `chat-core` | the API's request and response types, shared by both sides |
| `chat-server` | the server, over TCP or over SCION |
| `chat-client-core` | chat client functionality as a library |
| `chat-ui-ratatui` | a terminal UI on top of it |
| `chat-dev` | a SCION network on this machine, with the server in it |

`cargo doc_dx --open` renders them all.

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

## chat-server

`--transport tcp` serves the API as plain HTTP. It is a development mode.

`--transport scion` serves the same API over HTTP/3 in a SCION AS. It needs `--endhost-api` to find
the network, and `--auth-token-file` on the SNAP underlay. Both come from the network the server
joins, and `chat-dev` prints the whole command line. The server logs the certificate that clients
must pin, and the address it ended up reachable at:

```text
pin this certificate fingerprint=a4df3e47… cert=…/cert.pem
serving over scion addr=[2-ff00:0:212,127.0.0.1]:8443 server_name="localhost"
```

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

## chat-client-core

The typed API, the session, and the underlying transport:

- `TcpTransport` speaks plain HTTP to the server's `--transport tcp` mode
- `ScionTransport` speaks HTTP/3 over SCION to the server's `--transport scion` mode
- `MockTransport` answers from a script instead of a network, which is how a test produces what a
  real server cannot produce on demand

`ClientConfig::transport` picks one. `TransportKind::Scion` carries what SCION needs, so nothing
above it holds a field only one transport reads.

## chat-ui-ratatui

Three screens over `chat-client-core`: connect, sign in, chat. The screens draw and read keys;
`app.rs` holds every call to the client, so there is one place to look for how the SDK is used.

## chat-dev

Two autonomous systems joined by one link: the server in `2-ff00:0:212`, a client in
`1-ff00:0:132`. PocketSCION is a library rather than a daemon, so the network lives for exactly as
long as this process does. The underlay is SNAP and is not a choice — over SNAP an endpoint is
addressed at the address its tunnel observed, which is what a client behind a translation needs.

Almost nothing about the network can be written down in advance: the endhost APIs take whatever
ports are free, the token is minted per run, and the certificate is generated. So the network
describes itself. One line of JSON goes to standard output at startup, and the same document is
served at `GET /info` on `--control-port` (8099) over **plain TCP** — never over SCION, because a
client that cannot connect must still be able to learn why.

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

`endhost_api_url` is the client's AS and `server_endhost_api_url` is the server's. They are not
interchangeable. `ca_pem` is inline as well as on disk, because a client on an emulator cannot read
this filesystem. `auth_token` is minted per read and belongs to one client; `auth_token_file` holds
the server's own, which is deliberately a different one.

Standard error carries the logs and the same description in the shape a person reads. Ctrl+C stops
everything, and so does closing standard input, which is how a harness stops it.

### Running the server yourself

`--no-server` holds up only the network, and `chat_server_args` is the command that joins it:

```sh
cargo run -p chat-dev -- --no-server
cargo run -p chat-server -- $(curl -s http://127.0.0.1:8099/info | jq -r '.chat_server_args | join(" ")')
```

The data directory is shared on purpose: the server must present the certificate the description
describes.

### From another machine

`--bind-ip` moves every part of the network to an address another machine can reach:

```sh
cargo run -p chat-dev -- --bind-ip 192.168.1.20
```

It must be a real address on this machine. A wildcard is refused: the SNAP tunnel is dialled at
this address, and `0.0.0.0` names no host.

`--advertise-ip` publishes a different address than the one bound, for a client that reaches this
host another way. An Android emulator reaches the host's loopback as `10.0.2.2`, so nothing has to
move but what is published:

```sh
cargo run -p chat-dev -- --advertise-ip 10.0.2.2
```

It applies to the AS a client attaches to, not the one the server sits in.

## Development

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
