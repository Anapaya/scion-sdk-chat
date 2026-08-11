# scion-sdk-chat

Chat demo application using scion-sdk.

The workspace holds two crates: `chat-core`, the vocabulary shared by the server and every
client — the wire contract, the error codes, the protocol limits — and `chat-server`, the server
itself.

`chat-core` currently carries the wire contract: every JSON body the API accepts or returns, each
documented with the JSON it serializes to. Those examples are what the Kotlin and Swift clients
are implemented against, so read them there — `cargo doc_dx --open` — rather than re-deriving
them from the server.

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
