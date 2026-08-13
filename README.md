# scion-sdk-chat

Chat demo application using scion-sdk.

The workspace holds two crates: `chat-core`, the API's request and response types, and
`chat-server`, the server itself. `cargo doc_dx --open` renders both.

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
sqlite3 "$DB" < src/schema.sql
DATABASE_URL="sqlite://$DB" cargo sqlx prepare -- --all-targets
```

Forgetting to is a compile error. Metadata that has drifted from the schema is worse — it still
compiles — so CI runs the same commands with `--check`.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
