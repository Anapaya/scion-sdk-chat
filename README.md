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

`chat-dev` starts a SCION network on this machine and puts the chat server in it. The network runs
for as long as the process runs. The underlay is SNAP. Over SNAP the network addresses an endpoint
at the address its tunnel observed, which is what a client behind a translation needs.

### The topology

The network holds three autonomous systems. The server sits in the middle one.

```text
  1-ff00:0:132  (every client, at the bound address)
        |  iface 1 to 3
  2-ff00:0:212  (the chat server)
        |  iface 4 to 2
  2-ff00:0:222  (an Android emulator on this machine, at 10.0.2.2)
```

| AS | who attaches to it | published at |
| --- | --- | --- |
| `1-ff00:0:132` | every client, and the default | the bound address |
| `2-ff00:0:212` | the chat server | the bound address |
| `2-ff00:0:222` | an Android emulator on this machine | `10.0.2.2` |

`1-ff00:0:132` is the default AS. It publishes the address that `--bind-ip` binds, and every client
that can reach that address uses it. A terminal client on this machine uses it. A client on another
machine uses it, after you give `--bind-ip` an address that machine reaches.

An Android emulator on this machine is the one client that cannot use the default. The emulator
reaches the host at `10.0.2.2`, and inside the emulator `127.0.0.1` means the emulator itself. The
network publishes one address for each AS, and one AS holds one address, so the emulator gets
`2-ff00:0:222` as a fallback.

`--bind-ip` makes the fallback unnecessary. With a LAN address bound, the emulator reaches that
address like any other client, and it uses the default AS.

Two files build this:

| file | what it decides |
| --- | --- |
| [`crates/chat-dev/src/topology.rs`](crates/chat-dev/src/topology.rs) | the three ASes, the two links, and the endhost API and SNAP endpoint in each |
| [`crates/chat-dev/src/lib.rs`](crates/chat-dev/src/lib.rs) | the address each AS publishes, and the description `GET /info` answers with |

### Starting it

```sh
cargo run -p chat-dev
```

This one command starts the network and the chat server in one process. It starts the three ASes,
an endhost API and a SNAP endpoint for each, the control API on port 8099, and the server on port
8443. Ctrl+C stops all of it.

The network decides most of its addresses at startup. The endhost APIs take free ports, the network
makes a token for each run, and it generates the certificate. The network therefore describes
itself. It prints one line of JSON on standard output, and it serves the same document at
`GET /info` on `--control-port` (8099) over plain TCP. A client that failed to connect can still
read it.

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
  "auth_token": "eyJ0eXAiOiJKV1Qi...",
  "auth_token_file": "/tmp/.tmpTFIcwA/snap.token",
  "base_url": "https://localhost:8443",
  "target": "2-ff00:0:212,127.0.0.1",
  "ca_pem": "-----BEGIN CERTIFICATE-----\n...",
  "ca_path": "/tmp/.tmpTFIcwA/cert.pem",
  "ca_fingerprint": "a4df3e47...",
  "data_dir": "/tmp/.tmpTFIcwA",
  "chat_server_args": ["--transport", "scion", "--listen", "127.0.0.1:8443", "..."]
}
```

Each field, and what a client does with it:

| field | what a client does with it |
| --- | --- |
| `base_url` | the server's URL. The scheme is `https`, which serves `--transport scion` |
| `endhost_api_url` | the endhost API of the client's AS. The client finds SCION through it |
| `server_endhost_api_url` | the endhost API of the server's AS, for a server you start yourself |
| `client_isd_as` | the AS this description attaches a client to |
| `target` | the server's SCION address. This topology holds no TSAR records, so the client dials the address |
| `auth_token` | the SNAP token. Each read makes a new one, and it belongs to one client |
| `auth_token_file` | the server's own token, on disk |
| `ca_pem` / `ca_path` | the certificate to trust, as text and as a path |
| `chat_server_args` | the arguments that join a chat server you start yourself |

Each endhost API belongs to one AS. A client uses `endhost_api_url`, and a chat server uses
`server_endhost_api_url`. The description holds the certificate as text as well as a path, for a
client that reads a different filesystem.

`GET /info` answers with the description that matches the address the client asked at. A request to
`10.0.2.2` gets the emulator's AS. Every other request gets the default AS. Both descriptions name
the same server, the same certificate and the same SCION address.

Standard error carries the logs and the same description in the form a person reads. Ctrl+C stops
the process, and so does a close of standard input. A harness uses the second one.

### Connecting a client

Each field above has a flag and an environment variable, so one command puts the whole description
into the environment. Do these steps in a second terminal:

1. Read the description into the environment.
2. Start the terminal client.

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

Do both steps again in a third terminal for a second user. Their messages cross from
`1-ff00:0:132` to `2-ff00:0:212`.

Give each client its own token. The control plane keeps one tunnel for each subscriber. A second
client on the same token removes the tunnel of the first client. The first client then stops
without an error, and the server logs `wireguard error on incoming packet`. Run the `eval` again in
each terminal to give each client its own token.

### Running the server yourself

`--no-server` starts the network without the chat server:

```sh
cargo run -p chat-dev -- --no-server
```

`cargo run -p chat-server` then starts the chat server as its own process against the running
network. It is the same server that `chat-dev` starts. You can stop it, rebuild it and start it
again while the network stays up, which is what makes the second terminal worth it while you work
on the server. The `chat_server_args` field holds the command that joins it:

```sh
cargo run -p chat-server -- $(curl -s http://127.0.0.1:8099/info | jq -r '.chat_server_args | join(" ")')
```

Those arguments carry the server's own endhost API, its own token file, the listen address and the
data directory. Give the server the same data directory, so it presents the certificate the
description names.

### From another machine

The steps above bind to `127.0.0.1`, which only this machine reaches. `--bind-ip` moves every part
of the network to one address that another machine reaches. It moves the control API, all three
endhost APIs, the SNAP endpoints and the chat server.

```sh
cargo run -p chat-dev -- --bind-ip 192.168.1.20
```

Give a real address on this machine. `chat-dev` refuses a wildcard, because the SNAP tunnel dials
the bound address and `0.0.0.0` names no host. With a wildcard the network fails to start, and it
reports `error establishing SNAP tunnel`.

Every client then uses that one address, this machine included, so you give up the name
`localhost`. The address belongs to a real network, so it changes when DHCP moves you, and it
disappears when you work offline.

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
