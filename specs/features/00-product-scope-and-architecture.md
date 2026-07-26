# Product scope and architecture

## Goal

Provide the useful parts of Terminay's Tasks tab directly in a terminal:
recursive task discovery, understandable progress, explorable Markdown structure,
fast search, and live updates.

The tool is aimed at repositories whose plans live in Markdown rather than in a
separate task database. The Markdown files remain the source of truth.

## Terminay parity baseline

The behavior is based on the current Terminay implementations in:

- `src/components/file-viewer/tasks/parseTasks.ts`
- `src/components/file-viewer/tasks/taskView.tsx`
- `src/components/file-viewer/modes/TasksViewer.tsx`
- `src/components/folder-viewer/FolderTasksViewer.tsx`
- `src/components/folder-viewer/FolderPanel.tsx`

Parity includes:

- aggregate done, remaining, and files-complete counts;
- progress percentages and per-file/per-heading progress;
- heading-derived hierarchy and nested checkbox indentation;
- list and three-column Kanban views;
- all/remaining/done filters, free-text search, and progress/name sorting;
- collapse/expand at file and section level;
- recursive Markdown discovery with common generated directories ignored;
- live refresh while the interactive view is open.

Git diff completion badges and opening files inside Terminay are not required for
v1. They require host-editor integration and are tracked as possible follow-ups.

## Language decision

Use Rust.

Ratatui supplies the layout, widgets, styling primitives, Crossterm backend, and a
test backend suitable for deterministic UI tests. The `notify` ecosystem supplies
cross-platform filesystem events. Rust also produces a single native executable
with no runtime installation.

Bubble Tea is a credible Go alternative, but the Rust combination is preferred for
this project because the recursive task model can be shared without translation
between the parser, static renderer, and TUI, while Ratatui's buffer/test backend
supports precise rendering tests.

At implementation time, resolve mutually compatible current stable releases.
Do not copy version numbers from this planning document. Relevant upstream sources:

- <https://ratatui.rs/>
- <https://ratatui.rs/concepts/backends/>
- <https://docs.rs/notify/latest/notify/>
- <https://github.com/charmbracelet/bubbletea>

## Proposed module boundaries

The package is one Cargo package with a library and a thin binary:

```text
src/
  main.rs             process entry point and terminal-safe error reporting
  lib.rs              reusable public application surface
  cli.rs              argument parsing and mode selection
  model.rs            documents, sections, tasks, stats, stable keys
  markdown.rs         line-oriented Markdown task parser
  discover.rs         path validation, traversal, extensions, ignores
  snapshot.rs         scan orchestration and aggregate snapshot construction
  plain.rs            deterministic non-interactive renderer
  watch.rs            filesystem events, debounce, refresh messages
  tui/
    mod.rs            terminal lifecycle and event loop
    app.rs            state and state transitions
    input.rs          key bindings and search editing
    projection.rs     visible rows/cards derived from state
    ui.rs             responsive Ratatui rendering
    theme.rs          color and symbol capabilities
```

`main.rs` must contain no parsing or view logic. The library owns behavior so unit,
integration, and TUI-buffer tests do not need to spawn the binary except when
validating the CLI contract.

## Data flow

```text
path -> discover files -> parse documents -> immutable snapshot
                                      |
                        +-------------+-------------+
                        |                           |
                 plain projection             TUI projection
                 print and exit        input + watch events -> redraw
```

Scanning builds a complete replacement snapshot. The watcher never mutates the
current tree directly. A successful scan swaps the snapshot atomically; a failed
scan leaves the last good snapshot visible and adds an error status.

## Explicit non-goals for v1

- Editing or toggling Markdown checkboxes.
- Persisting UI state between separate invocations.
- Git diff-aware “completed in diff” counts.
- Editor, shell, clipboard, or OS file-manager actions.
- Remote filesystems or a background daemon.
- Configuration files. Repeated CLI ignore options are sufficient initially.
