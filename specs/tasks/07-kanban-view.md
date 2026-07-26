# Group 07 — Kanban view

Depends on: Group 06

Outcome: users can switch to a three-state Kanban projection comparable to
Terminay's Tasks tab, including search, sorting, and optional file grouping.

## Card model

- [x] Collect an Ungrouped card for root tasks.
- [x] Collect one card for every section that directly owns tasks.
- [x] Include stable key, title, heading breadcrumbs, file metadata, tasks, and
      shallow progress in each card.
- [x] Classify Not Started, Started, and Finished exactly from shallow progress.
- [x] Reuse unified search semantics and Progress/Name sorting.

## Interaction

- [x] Implement `v` List/Kanban toggle while preserving list filter/collapse state.
- [x] Hide list status filters in Kanban and restore the prior filter on return.
- [x] Implement visual-order card navigation and scrolling.
- [x] Implement `f` grouped-by-file boards and retain the setting across reloads.
- [x] Preserve selection by card key across sort, grouping, resize, and reload.

## Rendering

- [x] Render three labeled columns with card counts, breadcrumbs, titles, and
      progress.
- [x] Show filename in the global board and once per lane in grouped-by-file mode.
- [x] Add deliberate empty-column and no-search-result states.
- [x] Convert columns to selectable pages on narrow terminals.
- [x] Make Started/Finished/Not Started distinguishable without color.

## Verification

- [x] Unit test card collection, duplicate headings, root tasks, shallow stats,
      classification, search, and sorting.
- [x] Reducer-test view toggle, column/card navigation, and file grouping.
- [x] Snapshot wide, narrow, grouped, empty-column, search, and selected-card views.
