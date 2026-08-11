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

### Database queries

`chat-server`'s SQL is checked against the schema at compile time. What makes that work without a
database is committed in `.sqlx/`, so an ordinary build needs nothing extra. After adding or
changing a query, regenerate it — otherwise the build fails:

```sh
cargo install sqlx-cli --no-default-features --features sqlite
sqlite3 prepare.db < crates/chat-server/src/schema.sql
DATABASE_URL=sqlite://$PWD/prepare.db cargo sqlx prepare --workspace -- --all-targets
```

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
