# Group 06 — List exploration, search, and sorting

Depends on: Groups 02 and 05

Outcome: the main TUI view is an explorable, searchable multi-file task tree with
filters and sort behavior matching the feature specifications.

## Pure projections

- [x] Project snapshot documents/sections/tasks into a flat visible-row list.
- [x] Carry row kind, stable key, parent key, depth, stats, and render metadata.
- [x] Implement All, Remaining, and Done task predicates.
- [x] Keep full stats on file/section rows under a filter.
- [x] Implement unified case-insensitive search across file path, heading ancestry,
      and task labels.
- [x] Reveal matching ancestry during search without changing stored collapse state.
- [x] Apply recursive Progress and Name sorts without reordering tasks.
- [x] Produce explicit no-match messages.

## Interaction

- [x] Implement j/k, arrows, page movement, first/last, and non-wrapping bounds.
- [x] Implement Enter/Space/Right expansion and Left collapse/parent movement.
- [x] Implement `z` collapse/expand all.
- [x] Implement `/` search mode, editing keys, Enter, and two-stage Escape behavior.
- [x] Implement `1`/`2`/`3` filters and `s` sort toggle.
- [x] Reconcile selection by stable key, then nearest visual index after every
      projection change.
- [x] Keep the selected row inside the viewport.

## Rendering

- [x] Render file, section, and task row styles with clear nesting.
- [x] Render per-file/per-section counts and responsive progress tracks.
- [x] Render completion without relying only on color.
- [x] Elide long paths/labels safely at terminal cell boundaries.
- [x] Highlight the full selected row without hiding status.
- [x] Update toolbar/footer to show filter, sort, search query, and active shortcuts.

## Verification

- [x] Unit test projection combinations, including search plus status filter.
- [x] Reducer-test every key binding and modal conflict (`q` while searching).
- [x] Snapshot nested, collapsed, filtered, searching, sorted, selected, and
      no-match buffers.
- [x] Test stable selection/collapse across a newly inserted unrelated source line.
