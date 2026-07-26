# Group 05 — TUI shell and summary

Depends on: Groups 01 and 03

Outcome: `mdt --tui PATH` opens and safely closes a responsive task dashboard with
the aggregate summary, viewport shell, and status/footer regions.

## Event loop and state

- [x] Add Ratatui and select its Crossterm backend.
- [x] Define the application model, message/event enum, reducer-style state
      transitions, and draw boundary.
- [x] Read keyboard and resize events without blocking future scan/watch messages.
- [x] Enter the alternate screen only after target validation succeeds.
- [x] Quit on `q`/`Ctrl-C` and restore every terminal mode.
- [x] Keep all scanning outside the draw/input path.

## Responsive shell

- [x] Implement title/root/status line, summary, toolbar, content viewport, and
      contextual footer regions.
- [x] Render overall percentage, counts, files complete, and aggregate progress bar.
- [x] Render scanned files/directories/ignored metadata and current scan state.
- [x] Implement wide, medium, narrow, and terminal-too-small layouts.
- [x] Implement semantic theme roles and color-capability fallbacks.
- [x] Implement Unicode and ASCII glyph sets.
- [x] Handle rapid resize and zero-area rectangles without panic.

## States and help

- [x] Render initial scanning, no tasks, partial warning, fatal error, and ready
      states.
- [x] Add `?` help overlay with all implemented bindings and a read-only notice.
- [x] Add `r` as a placeholder manual scan action before watch integration.
- [x] Preserve underlying selection when opening/closing help.

## Verification

- [x] Add TestBackend buffer snapshots for every responsive width and major state.
- [x] Add reducer tests for quit, resize, help, and manual refresh messages.
- [x] Exercise panic/error teardown in a pseudo-terminal or equivalent harness.
