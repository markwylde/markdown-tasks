# Group 09 — Hardening, documentation, and release

Depends on: Groups 01–08

Outcome: v0.1.0 is documented, portable, tested, and packaged as an installable
`mdt` binary.

## Product polish

- [ ] Audit every screen at wide, medium, narrow, light, dark, colorless, and ASCII
      terminal capabilities.
- [ ] Audit long Unicode labels/paths, combining characters, and terminal cell width.
- [ ] Profile the 10,000-file/1,000-Markdown fixture and remove measured bottlenecks.
- [ ] Confirm no scan, parse, or render path panics on malformed input.
- [ ] Confirm every completion/warning/error state is understandable without color.
- [ ] Decide whether mouse scrolling/selection is reliable enough for v0.1.0;
      otherwise leave it disabled and documented as keyboard-first.

## Documentation

- [ ] Write the root README with installation, CLI examples, syntax, extensions,
      ignores, keys, read-only guarantee, and watcher caveats.
- [ ] Capture a representative terminal screenshot or recording using the
      multi-file fixture.
- [ ] Add `CONTRIBUTING.md` with local checks and golden/snapshot update workflow.
- [ ] Add `CHANGELOG.md` and record v0.1.0 behavior and known limitations.
- [ ] Keep `mdt --help` examples aligned with README and specs.

## Supply chain and CI

- [ ] Pin a minimum supported Rust version and test it.
- [ ] Add `cargo deny` policy for advisories, licenses, bans, and sources.
- [ ] Run fmt, clippy with warnings denied, all tests, and release builds in CI.
- [ ] Add cross-platform binary release jobs and checksum generation.
- [ ] Verify the installed artifact is named `mdt` on Unix and `mdt.exe` on Windows.

## Release acceptance

- [ ] Run `mdt specs/tasks` and archive the expected plain output.
- [ ] Run `mdt --tui specs/tasks` and verify list, search, filters, sorting, Kanban,
      grouping, collapse, refresh, live updates, help, resize, and quit.
- [ ] Verify create/modify/delete/rename refresh on every supported platform.
- [ ] Verify stdout/stderr and exit codes in a CI-like non-TTY environment.
- [ ] Verify clean terminal restoration after normal quit, Ctrl-C, scan error,
      watcher error, and forced panic test.
- [ ] Tag v0.1.0 only after all feature-spec acceptance criteria are satisfied.
