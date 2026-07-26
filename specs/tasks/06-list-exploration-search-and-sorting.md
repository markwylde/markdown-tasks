# Group 06 — List exploration, search, and sorting

Depends on: Groups 02 and 05

Outcome: the main TUI view is an explorable, searchable multi-file task tree with
filters and sort behavior matching the feature specifications.

## Pure projections

- [ ] Project snapshot documents/sections/tasks into a flat visible-row list.
- [ ] Carry row kind, stable key, parent key, depth, stats, and render metadata.
- [ ] Implement All, Remaining, and Done task predicates.
- [ ] Keep full stats on file/section rows under a filter.
- [ ] Implement unified case-insensitive search across file path, heading ancestry,
      and task labels.
- [ ] Reveal matching ancestry during search without changing stored collapse state.
- [ ] Apply recursive Progress and Name sorts without reordering tasks.
- [ ] Produce explicit no-match messages.

## Interaction

- [ ] Implement j/k, arrows, page movement, first/last, and non-wrapping bounds.
- [ ] Implement Enter/Space/Right expansion and Left collapse/parent movement.
- [ ] Implement `z` collapse/expand all.
- [ ] Implement `/` search mode, editing keys, Enter, and two-stage Escape behavior.
- [ ] Implement `1`/`2`/`3` filters and `s` sort toggle.
- [ ] Reconcile selection by stable key, then nearest visual index after every
      projection change.
- [ ] Keep the selected row inside the viewport.

## Rendering

- [ ] Render file, section, and task row styles with clear nesting.
- [ ] Render per-file/per-section counts and responsive progress tracks.
- [ ] Render completion without relying only on color.
- [ ] Elide long paths/labels safely at terminal cell boundaries.
- [ ] Highlight the full selected row without hiding status.
- [ ] Update toolbar/footer to show filter, sort, search query, and active shortcuts.

## Verification

- [ ] Unit test projection combinations, including search plus status filter.
- [ ] Reducer-test every key binding and modal conflict (`q` while searching).
- [ ] Snapshot nested, collapsed, filtered, searching, sorted, selected, and
      no-match buffers.
- [ ] Test stable selection/collapse across a newly inserted unrelated source line.
