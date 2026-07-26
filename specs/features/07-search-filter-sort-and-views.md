# Search, filters, sorting, and views

## Search

`/` focuses an inline search field. Input is incremental and case-insensitive.
Backspace edits; `Ctrl-W` deletes the previous word; `Ctrl-U` clears the field;
Enter returns focus to results while retaining the query.

A document is visible when its path, a heading in its ancestry, or a descendant
task label matches. A task-label match shows the task and all required ancestor
context. A file/heading match shows its descendant tasks subject to the active
status filter. This deliberately gives list and Kanban search the same semantics.

Search never changes progress counts. Show `No tasks match "…" ` when the
projection is empty.

## Status filter

The list view supports:

- All: every task.
- Remaining: unchecked tasks only.
- Done: checked tasks only.

File and section rows remain visible only if they contain a matching task. Their
badges continue to show full, unfiltered stats.

Kanban status already encodes progress, so the All/Remaining/Done toolbar is hidden
in Kanban, matching Terminay. The last selected filter is retained and restored
when returning to List.

## Sorting

Progress and Name sorts apply recursively to:

- documents;
- sibling sections;
- Kanban cards.

Tasks within a section remain in source order. Sort choice persists across view
changes and reloads.

## List view

The list view presents:

1. document rows;
2. nested heading rows;
3. task rows owned by each heading.

Rows show completion/remaining glyphs, counts, and a progress track when space
allows. Task indentation reflects checkbox indentation in addition to heading
indentation.

## Kanban view

A card is the lowest grouping that directly owns one or more tasks:

- tasks before a heading create an `Ungrouped` card;
- every section directly owning tasks creates a card;
- child sections create their own cards.

Columns:

- Not Started: `completed == 0`;
- Started: `0 < completed < total`;
- Finished: `completed >= total`.

Each card shows breadcrumb headings, optional filename, title, and `done/total`
progress. `f` toggles one global three-column board versus separate boards grouped
by file. Arrow navigation follows visual card order. On narrow terminals, columns
become horizontally selected pages rather than rendering unusably thin cards.

## Projection purity

Search, filtering, sorting, collapse, and view choice derive visible rows/cards
from the immutable scan snapshot. Projection functions must be independently unit
tested and must not mutate the parsed model.
