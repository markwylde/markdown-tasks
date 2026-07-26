# Navigation and exploration

## Focus model

The content viewport is the normal focus. Search input and help overlay are modal
states with explicit entry/exit; the toolbar is controlled by shortcuts rather
than requiring tabbing through every control.

## Default key map

| Key | Action |
| --- | --- |
| `q`, `Ctrl-C` | Quit (except `q` types into active search) |
| `j`, `Down` | Select next visible row/card |
| `k`, `Up` | Select previous visible row/card |
| `PageDown`, `Ctrl-D` | Move down by a viewport/page |
| `PageUp`, `Ctrl-U` | Move up by a viewport/page |
| `g`, `Home` | First visible item |
| `G`, `End` | Last visible item |
| `Enter`, `Space`, `Right` | Expand selected file/section |
| `Left` | Collapse selected node, then move to its parent |
| `z` | Toggle collapse/expand all |
| `/` | Enter search mode |
| `Esc` | Leave modal; clear active search on a second press |
| `1`, `2`, `3` | All, remaining, done |
| `v` | Toggle List/Kanban |
| `s` | Toggle Progress/Name sort |
| `f` | Toggle Kanban grouping by file |
| `r` | Request immediate rescan |
| `?` | Toggle help |

Footer help shows the most relevant subset and the overlay documents all bindings.

## Tree behavior

- File and section rows are expandable. Task rows are leaves.
- Files and sections start expanded on the first launch, matching Terminay.
- Collapse state survives filtering, sorting, resizing, and successful reloads.
- `z` collapses all when any expandable node is open; otherwise it expands all.
- Search temporarily reveals every matching branch but does not erase stored
  collapse state. Clearing search restores the previous collapse state.
- Section stats always reflect the complete underlying section, not only visible
  filtered tasks.

## Selection and scrolling

- Exactly one visible row/card is selected when results exist.
- The selected item is always scrolled into view.
- Reprojection after filter/search/sort/reload preserves the same stable key when
  possible, otherwise the nearest previous visual index.
- Scrolling does not wrap at the top or bottom.
- Selection style spans the usable row width and does not obscure checkbox state.

## Read-only guarantee

Space and Enter only expand/collapse. They never alter a checkbox or write a file.
The help overlay and README must say that v1 is read-only.

Mouse support is optional polish after the keyboard behavior is complete. If
enabled, scrolling and single-click selection must not change Markdown.
