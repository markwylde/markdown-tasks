# Markdown task model

## Supported source syntax

A task is a Markdown list item whose marker is followed by `[ ]`, `[x]`, or `[X]`.
Bullet and ordered list markers are supported:

```markdown
- [ ] todo
* [x] done
+ [X] also done
1. [ ] ordered
2) [x] ordered with parenthesis
```

Leading spaces and tabs determine task nesting depth. A tab counts as two spaces
for compatibility with Terminay's current parser. Depth is derived from increasing
indent widths among consecutive task lines and resets when a heading is read.

ATX headings from `#` through `######` create sections. Heading levels determine
the section tree; skipped levels are legal. Tasks before the first heading belong
to an implicit root group.

Checkbox-looking text inside fenced code blocks is ignored. Backtick and tilde
fences are supported. Fences may be indented. Checkbox-looking prose that is not a
list item is ignored.

## Labels

- Preserve the source label after trimming outer whitespace.
- Preserve inline Markdown characters in the model.
- Collapse labels to a single terminal line when rendering.
- An empty label is valid and renders as `(untitled task)`.
- URLs and inline markup are display text in v1; they are not interactive.

## Core model

```text
WorkspaceSnapshot
  root_path
  documents[]
  aggregate_stats
  scan_stats
  warnings[]

Document
  key
  absolute_path
  relative_path
  root Section
  stats

Section
  key
  title? / level
  tasks[]
  children[]
  stats

Task
  key
  label
  checked
  depth
  line_number
```

Stats contain `total`, `completed`, and derived `remaining`. A node is complete
only when `total > 0 && completed == total`. Percent is `0` for no tasks and
otherwise the nearest integer to `completed / total * 100`.

## Stable identity

UI selection and collapse state should survive ordinary reloads. Use keys based on:

- document: normalized root-relative path;
- section: document key plus heading ancestry, normalized title, and same-title
  occurrence index;
- task: document key plus section key, normalized label, and same-label occurrence
  index.

Line number remains metadata but is not the sole identity. If an item disappears,
selection moves to the nearest surviving visible row.

## Sort semantics

Name sorting is case-insensitive, numeric-aware, ascending by displayed path/title.

Progress sorting follows Terminay:

1. higher rounded completion percentage first;
2. fewer remaining tasks first;
3. name ascending.

The original source order is retained in the parsed model. Sorting is a view
projection and never changes source order or stable keys.

## Parser tests

Fixtures must cover CRLF/LF, every accepted marker, checked casing, nested
indentation, repeated labels/headings, skipped heading levels, root tasks, fences,
empty labels, Unicode, malformed checkboxes, and documents with no tasks.
