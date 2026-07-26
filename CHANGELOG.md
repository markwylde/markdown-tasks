# Changelog

All notable changes to this project are documented here.

## 0.1.0

- Added deterministic one-shot reports for Markdown task files and directories.
- Added a read-only Ratatui explorer with list and Kanban views.
- Added hierarchical progress, search, status filters, sorting, and collapse.
- Added recursive discovery, practical default ignores, and partial-scan warnings.
- Added debounced live filesystem watching and manual refresh.
- Added cross-platform terminal cleanup and responsive layouts.

Known limitation: native filesystem events may be unreliable on some network or
cross-operating-system mounts; use `r` for manual refresh.
