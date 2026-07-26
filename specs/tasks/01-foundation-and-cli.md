# Group 01 — Foundation and CLI

Depends on: nothing

Outcome: a tested Rust package installs an `mdt` binary, parses the agreed command
line, and safely dispatches between plain and TUI modes.

## Repository foundation

- [ ] Create a Cargo package with `src/lib.rs` and thin `src/main.rs`.
- [ ] Set package metadata, Rust edition, minimum supported Rust version, license,
      binary name `mdt`, and repository metadata.
- [ ] Add `.gitignore`, formatting settings, and a CI workflow for fmt, clippy,
      and tests on macOS, Linux, and Windows.
- [ ] Add error types separating invalid input, fatal scan failures, partial scan
      warnings, terminal setup failures, and watcher failures.
- [ ] Establish modules from the architecture spec without filling them with
      speculative abstractions.

## CLI

- [ ] Add Clap derive definitions for `[PATH]`, `--tui`, repeatable `--ignore`,
      `--no-default-ignore`, and `--color`.
- [ ] Default `PATH` to `.` and normalize it without losing the display path.
- [ ] Implement help, version, output-stream, and exit-code behavior.
- [ ] Reject an unsupported explicit file before entering either renderer.
- [ ] Reject `--tui` when stdin or stdout is not an interactive terminal.
- [ ] Honor `NO_COLOR` and explicit color precedence.
- [ ] Dispatch non-TUI mode without reading stdin or constructing a watcher.

## Terminal safety skeleton

- [ ] Add a scoped terminal guard that can restore raw mode, alternate screen,
      cursor, and mouse state.
- [ ] Install panic/signal restoration without swallowing the original diagnostic.
- [ ] Prove normal and error teardown with focused tests or a pseudo-terminal test.

## Verification

- [ ] Add binary tests for default path, explicit path, help, version, malformed
      flags, invalid path, unsupported extension, and TTY rejection.
- [ ] Confirm `cargo run -- --help` names the installed command `mdt`.
- [ ] Document how later task groups plug into the mode dispatcher.
