<!-- Copyright 2026 Anapaya Systems -->

# Code that the SCION SDK guide quotes

The [SCION SDK guide](https://learn.anapaya.net/docs/academy/scion-sdk/) quotes regions of this
repository, from a tagged version. A pair of comments marks each region in its source file:

```rust
// ANCHOR: build-stack
let mut builder = ScionStackBuilder::new().with_endhost_api(endhost_api);
// ANCHOR_END: build-stack
```

[anchors.txt](anchors.txt) lists each region as `<path>#<anchor>`.

- Keep the markers when you change the code between them. A removed or renamed marker breaks the
  guide at the next tag.
- To quote a new region, add the markers and add 1 line to `anchors.txt`.
- [check-anchors.sh](check-anchors.sh) checks the list against the markers. CI runs it on every
  pull request.
