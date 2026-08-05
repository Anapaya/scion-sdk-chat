# scion-sdk-chat

Chat demo application using scion-sdk.

The workspace holds two crates: `chat-core`, the vocabulary shared by the server and every
client — the wire contract, the error codes, the protocol limits — and `chat-server`, the server
itself. Both are placeholders until the following tickets fill them in.

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

## License

Licensed under the [Apache License, Version 2.0](LICENSE).
