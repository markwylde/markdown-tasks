# `mdt` implementation task plan

These files turn the [feature specifications](../features/README.md) into grouped,
ordered work. Every checkbox should be completed in a worktree, with tests in the
same change as the behavior.

## Delivery order

```text
01 foundation
  ├─> 02 model/parser ─> 03 discovery/snapshots ─> 04 plain report
  │                                  │
  │                                  └─> 05 TUI shell/summary
  │                                           └─> 06 list/navigation/search
  │                                                    └─> 07 Kanban
  │                                                             │
  └─────────────────────────────────────────────────────────────> 08 watch
                                                                  └─> 09 release
```

## Groups

1. [Foundation and CLI](./01-foundation-and-cli.md)
2. [Task model and Markdown parser](./02-task-model-and-markdown-parser.md)
3. [Discovery and snapshots](./03-discovery-and-snapshots.md)
4. [Non-interactive report](./04-non-interactive-report.md)
5. [TUI shell and summary](./05-tui-shell-and-summary.md)
6. [List exploration, search, and sorting](./06-list-exploration-search-and-sorting.md)
7. [Kanban view](./07-kanban-view.md)
8. [Live watch and reload](./08-live-watch-and-reload.md)
9. [Hardening, documentation, and release](./09-hardening-documentation-and-release.md)

## Working agreement

- A group is complete only when its tests and relevant docs pass.
- Keep the parsed snapshot immutable; all UI behavior is projection/state.
- Preserve the last good snapshot across recoverable refresh failures.
- Do not add checkbox editing during v1 implementation.
- Run formatting, linting, and tests before marking any group complete.
