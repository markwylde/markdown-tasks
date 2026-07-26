# Quality, portability, and release

## Supported platforms

The first release targets:

- macOS on Apple Silicon and Intel;
- Linux on x86_64 and ARM64;
- Windows x86_64 in a modern terminal.

Core parsing and plain output must not assume a Unix path separator. TUI behavior
targets terminals supported by Crossterm.

## Test layers

1. Unit tests for parser, stats, stable keys, sort, search, filter, and cards.
2. Filesystem integration tests using temporary directories for traversal,
   ignores, symlinks, warnings, and refresh snapshots.
3. Plain-output golden tests.
4. Ratatui `TestBackend` buffer snapshots at wide, medium, narrow, empty, error,
   help, search, list, and Kanban states.
5. Reducer/state-transition tests for key input and synthetic watcher messages.
6. Binary integration tests for help, version, exit status, stdout/stderr, TTY
   rejection, and terminal restoration where feasible.

Tests must not depend on wall-clock sleeps. Debounce and “just now” behavior use an
injectable clock or directly driven state transitions.

## Engineering checks

CI runs:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo deny check
```

Add dependency/license policy only after the initial dependency graph exists.
Keep unsafe code out of project modules. Treat malformed Markdown and filesystem
events as data, not invariants.

## Documentation

The root README must include:

- installation from Cargo and release artifacts;
- the two primary invocations;
- a screenshot or terminal recording of the TUI;
- key bindings;
- supported Markdown syntax and extensions;
- ignore behavior;
- read-only guarantee;
- watcher limitations and manual refresh;
- shell completion generation if added.

`mdt --help` remains the authoritative compact CLI reference.

## Packaging

- Binary name: `mdt`.
- Package/repository name may remain `markdown-tasks` if the crates.io name `mdt`
  is unavailable; the installed binary is still `mdt`.
- Build stripped release binaries for supported targets.
- Generate checksums and a changelog entry.
- Do not promise Homebrew, Scoop, or other package managers until release
  automation and ownership are established.

## Release acceptance

Before v0.1.0:

- all feature-spec acceptance behavior is implemented;
- a fixture resembling Terminay's multi-file task plans is demonstrated;
- no known terminal-corruption path remains;
- watch-create/modify/delete/rename works on macOS, Linux, and Windows CI or is
  manually recorded where hosted CI cannot provide reliable native events;
- plain output is stable enough to document as human-readable, but not yet a
  versioned machine format.
