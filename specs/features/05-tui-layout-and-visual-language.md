# TUI layout and visual language

## Screen regions

The default list view uses the entire alternate screen:

```text
┌ mdt ─ specs/tasks ───────────────────────────────────────── live • up to date ┐
│  97%  221 done   8 remaining   2/4 files complete                           │
│  ████████████████████████████████████████████████████████░░                 │
│  4 markdown files · 1 folder watched                                        │
├ List  Kanban ─ All  Remaining  Done ─ / search ─ Sort: Progress ─ Collapse ─┤
│▾ ✓ PHASE18_MEDIA_SPLIT.md                                  58/58 ██████████ │
│  ▾ PHASE18 — Split Persistent Disks From Reusable Media    58/58 ██████████ │
│    ▾ Work streams                                          58/58 ██████████ │
│      ▸ 1. Data model and migrations                          7/7 ██████████ │
│▸ ✓ PHASE19_LOAD_BALANCERS.md                               53/53 ██████████ │
│▸   UI_UX_OVERHAUL.md                                       24/25 █████████░ │
├ j/k move  enter expand  / search  v view  r refresh  ? help  q quit ───────┤
```

This is a terminal interpretation, not a pixel-for-pixel copy. It should preserve
the visual hierarchy, restrained dark-theme accenting, compact progress tracks,
and obvious completion state shown in the Terminay screen.

## Summary

Display:

- overall rounded percentage;
- completed and remaining counts;
- complete task documents over all task documents;
- a horizontal aggregate progress gauge;
- scan/watcher status and counts.

Do not attempt a circular progress ring in text cells. The large percentage plus
horizontal gauge communicates the same information more clearly in a TUI.

## Responsive behavior

- Wide (>= 100 columns): full summary, metadata, labels, counts, progress bars.
- Medium (60–99): compact summary and shorter progress bars; elide long paths in
  the middle while preserving filenames.
- Narrow (40–59): one-line summary, counts without per-row bars, abbreviated help.
- Below 40x10: show a centered “terminal too small” message with current and
  recommended dimensions; continue accepting resize and quit events.

Height controls viewport size. The header and footer remain visible while the
content scrolls. Rendering must never panic on zero-sized or rapidly resized areas.

## Theme and capability fallback

Use semantic roles rather than embedded colors:

- accent/progress;
- success/completed;
- warning/remaining;
- selected/focused;
- muted metadata;
- error.

Detect color capability through the terminal backend where possible. Completion
must never be conveyed by color alone: glyphs, checkbox state, counts, and text
remain present. Provide ASCII substitutes for all Unicode drawing characters.

## Status overlays

`?` opens a help overlay listing current key bindings. Fatal scan state, no tasks,
no matches, refresh-in-progress, and last-refresh-error each have deliberate empty
or status presentations. Overlays must not destroy the underlying selection.
