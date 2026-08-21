# scion-sdk-chat

Chat demo application using scion-sdk.

The workspace holds four crates:

- `chat-core`, the API's request and response types
- `chat-server`, the server
- `chat-client-core`, chat client functionality as library
- `chat-ui-dioxus`, a desktop UI on top of it

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

- `TcpTransport` speaks plain HTTP to the server's `--transport tcp` mode
- `MockTransport` answers from a script instead of a network, which is how a test produces what a
  real server cannot produce on demand

## chat-ui-dioxus guide

Three screens over `chat-client-core` — connect, sign in, chat — against a server in
`--transport tcp` mode:

```sh
cargo run -p chat-server -- --transport tcp --listen 127.0.0.1:8080 --data-dir ./data
cargo run -p chat-ui-dioxus
```

Register signs in with the account it created. Click a room to switch, Enter sends, and
`/room name` creates a room.

The window is the system WebView, so `cargo run` is all it takes — the `dx` CLI is for web builds
and hot reloading, neither of which this POC uses.

`app.rs` holds every call to the client and the state the screens read; `screens.rs` renders it.

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
