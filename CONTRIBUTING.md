# Contributing

Use a Git worktree for every change. Do not edit the repository's primary
checkout directly.

## Local checks

Install stable Rust plus `rustfmt`, `clippy`, and `cargo-deny`, then run:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo deny check
```

Parser, projection, and watcher state tests should be deterministic. Avoid
wall-clock sleeps; inject time or drive state transitions directly.

Ratatui rendering tests use its `TestBackend`. When an intentional layout change
alters expected buffers, inspect every changed width/state rather than accepting
snapshots blindly. Plain-output golden fixtures are a human-readable interface
and should not include timestamps, machine-specific paths, or terminal escapes.

All behavior changes should update the matching file in `specs/features` and tick
only checklist entries that the implementation and tests actually satisfy.
