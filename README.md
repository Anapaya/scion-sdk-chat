# scion-sdk-chat

Chat demo application using scion-sdk.

The workspace holds two crates:

- `chat-core`, the API's request and response types
- `chat-server`, the server

`cargo doc_dx --open` renders both.

## chat-server guide

`--transport tcp` serves the API as plain HTTP, with no TLS and no SCION, so every endpoint is
reachable with `curl`. It is a development mode.

```sh
cargo run -p chat-server -- --transport tcp --listen 127.0.0.1:8080 --data-dir ./data
```

Every flag has a `CHAT_*` environment fallback:

| Flag | Env | Default | |
|---|---|---|---|
| `--transport` | `CHAT_TRANSPORT` | `scion` | `scion` or `tcp` |
| `--listen` | `CHAT_LISTEN` | `0.0.0.0:8443` | address to bind |
| `--data-dir` | `CHAT_DATA_DIR` | **required** | holds `chat.db` and `jwt.secret`; created if absent |
| `--max-accounts` | `CHAT_MAX_ACCOUNTS` | `500` | accounts accepted |
| `--max-rooms` | `CHAT_MAX_ROOMS` | `100` | rooms accepted |
| `--max-message-bytes` | `CHAT_MAX_MESSAGE_BYTES` | `4096` | largest message body |
| `--token-expiry-days` | `CHAT_TOKEN_EXPIRY_DAYS` | `7` | how long a login lasts |
| `--endhost-api` | `CHAT_ENDHOST_API` | — | how to reach the SCION network |
| `--auth-token-file` | `CHAT_AUTH_TOKEN_FILE` | — | SNAP token, on that underlay only |

`cargo run -p chat-server -- --help` prints the same list from the binary itself.

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
