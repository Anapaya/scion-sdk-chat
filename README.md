# scion-sdk-chat

Chat demo application using scion-sdk.

The workspace holds two crates: `chat-core`, the request and response types the server and every
client share, and `chat-server`, the server itself. `cargo doc_dx --open` renders both.

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
the schema. `.sqlx/` is what it was told, one JSON file per query.

It is committed so that a clone builds and runs with no database and no tooling — `cargo test` and
`cargo run` need neither `DATABASE_URL` nor `sqlx-cli`. Without it the crate does not compile.

You only need `sqlx-cli` to add or change a query, which requires regenerating it:

```sh
cargo install --version 0.9.0 sqlx-cli --no-default-features --features sqlite  # match Cargo.toml
rm -f prepare.db   # the schema is all CREATE TABLE IF NOT EXISTS, so a stale file stays stale
sqlite3 prepare.db < crates/chat-server/src/schema.sql
DATABASE_URL=sqlite://$PWD/prepare.db cargo sqlx prepare --workspace -- --all-targets
```

Forgetting to is a compile error. Metadata that has drifted from the schema is worse — it still
compiles — so CI runs the same commands with `--check`.

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
