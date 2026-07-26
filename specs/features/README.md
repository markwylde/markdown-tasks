# `mdt` feature specifications

This directory defines the first releasable version of `mdt`: a read-only Markdown
task explorer inspired by Terminay's file-viewer Tasks tab.

## Product decisions

- Implementation language: Rust.
- Interactive UI: Ratatui using its Crossterm backend.
- Invocation: `mdt [--tui] [PATH]`.
- Default mode is a deterministic, non-interactive report that prints and exits.
- `--tui` starts a live, full-screen explorer and watches the target for changes.
- Files are never modified in v1. Checkboxes report Markdown state; they are not
  controls for editing source files.
- Both a Markdown file and a directory are valid targets. `PATH` defaults to `.`.

## Specifications

1. [Product scope and architecture](./00-product-scope-and-architecture.md)
2. [CLI contract](./01-cli-contract.md)
3. [Markdown task model](./02-markdown-task-model.md)
4. [Discovery, scanning, and ignores](./03-discovery-scanning-and-ignores.md)
5. [Non-interactive report](./04-non-interactive-report.md)
6. [TUI layout and visual language](./05-tui-layout-and-visual-language.md)
7. [Navigation and exploration](./06-navigation-and-exploration.md)
8. [Search, filters, sorting, and views](./07-search-filter-sort-and-views.md)
9. [Live watch and reload](./08-live-watch-and-reload.md)
10. [Quality, portability, and release](./09-quality-portability-and-release.md)

## v1 definition of done

`mdt specs/tasks` prints an expanded snapshot and exits successfully.
`mdt --tui specs/tasks` opens a responsive explorer with aggregate progress,
expandable files and headings, list and Kanban views, search, status filters,
sorting, manual refresh, and automatic reload after filesystem changes.
