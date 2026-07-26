# Group 05 — TUI shell and summary

Depends on: Groups 01 and 03

Outcome: `mdt --tui PATH` opens and safely closes a responsive task dashboard with
the aggregate summary, viewport shell, and status/footer regions.

## Event loop and state

- [ ] Add Ratatui and select its Crossterm backend.
- [ ] Define the application model, message/event enum, reducer-style state
      transitions, and draw boundary.
- [ ] Read keyboard and resize events without blocking future scan/watch messages.
- [ ] Enter the alternate screen only after target validation succeeds.
- [ ] Quit on `q`/`Ctrl-C` and restore every terminal mode.
- [ ] Keep all scanning outside the draw/input path.

## Responsive shell

- [ ] Implement title/root/status line, summary, toolbar, content viewport, and
      contextual footer regions.
- [ ] Render overall percentage, counts, files complete, and aggregate progress bar.
- [ ] Render scanned files/directories/ignored metadata and current scan state.
- [ ] Implement wide, medium, narrow, and terminal-too-small layouts.
- [ ] Implement semantic theme roles and color-capability fallbacks.
- [ ] Implement Unicode and ASCII glyph sets.
- [ ] Handle rapid resize and zero-area rectangles without panic.

## States and help

- [ ] Render initial scanning, no tasks, partial warning, fatal error, and ready
      states.
- [ ] Add `?` help overlay with all implemented bindings and a read-only notice.
- [ ] Add `r` as a placeholder manual scan action before watch integration.
- [ ] Preserve underlying selection when opening/closing help.

## Verification

- [ ] Add TestBackend buffer snapshots for every responsive width and major state.
- [ ] Add reducer tests for quit, resize, help, and manual refresh messages.
- [ ] Exercise panic/error teardown in a pseudo-terminal or equivalent harness.
