# Group 01 — Foundation and CLI

Depends on: nothing

Outcome: a tested Rust package installs an `mdt` binary, parses the agreed command
line, and safely dispatches between plain and TUI modes.

## Repository foundation

- [x] Create a Cargo package with `src/lib.rs` and thin `src/main.rs`.
- [x] Set package metadata, Rust edition, minimum supported Rust version, license,
      binary name `mdt`, and repository metadata.
- [x] Add `.gitignore`, formatting settings, and a CI workflow for fmt, clippy,
      and tests on macOS, Linux, and Windows.
- [x] Add error types separating invalid input, fatal scan failures, partial scan
      warnings, terminal setup failures, and watcher failures.
- [x] Establish modules from the architecture spec without filling them with
      speculative abstractions.

## CLI

- [x] Add Clap derive definitions for `[PATH]`, `--tui`, repeatable `--ignore`,
      `--no-default-ignore`, and `--color`.
- [x] Default `PATH` to `.` and normalize it without losing the display path.
- [x] Implement help, version, output-stream, and exit-code behavior.
- [x] Reject an unsupported explicit file before entering either renderer.
- [x] Reject `--tui` when stdin or stdout is not an interactive terminal.
- [x] Honor `NO_COLOR` and explicit color precedence.
- [x] Dispatch non-TUI mode without reading stdin or constructing a watcher.

## Terminal safety skeleton

- [x] Add a scoped terminal guard that can restore raw mode, alternate screen,
      cursor, and mouse state.
- [x] Install panic/signal restoration without swallowing the original diagnostic.
- [x] Prove normal and error teardown with focused tests or a pseudo-terminal test.

## Verification

- [x] Add binary tests for default path, explicit path, help, version, malformed
      flags, invalid path, unsupported extension, and TTY rejection.
- [x] Confirm `cargo run -- --help` names the installed command `mdt`.
- [x] Document how later task groups plug into the mode dispatcher.
