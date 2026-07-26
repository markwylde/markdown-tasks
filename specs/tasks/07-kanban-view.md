# Group 07 — Kanban view

Depends on: Group 06

Outcome: users can switch to a three-state Kanban projection comparable to
Terminay's Tasks tab, including search, sorting, and optional file grouping.

## Card model

- [ ] Collect an Ungrouped card for root tasks.
- [ ] Collect one card for every section that directly owns tasks.
- [ ] Include stable key, title, heading breadcrumbs, file metadata, tasks, and
      shallow progress in each card.
- [ ] Classify Not Started, Started, and Finished exactly from shallow progress.
- [ ] Reuse unified search semantics and Progress/Name sorting.

## Interaction

- [ ] Implement `v` List/Kanban toggle while preserving list filter/collapse state.
- [ ] Hide list status filters in Kanban and restore the prior filter on return.
- [ ] Implement visual-order card navigation and scrolling.
- [ ] Implement `f` grouped-by-file boards and retain the setting across reloads.
- [ ] Preserve selection by card key across sort, grouping, resize, and reload.

## Rendering

- [ ] Render three labeled columns with card counts, breadcrumbs, titles, and
      progress.
- [ ] Show filename in the global board and once per lane in grouped-by-file mode.
- [ ] Add deliberate empty-column and no-search-result states.
- [ ] Convert columns to selectable pages on narrow terminals.
- [ ] Make Started/Finished/Not Started distinguishable without color.

## Verification

- [ ] Unit test card collection, duplicate headings, root tasks, shallow stats,
      classification, search, and sorting.
- [ ] Reducer-test view toggle, column/card navigation, and file grouping.
- [ ] Snapshot wide, narrow, grouped, empty-column, search, and selected-card views.
