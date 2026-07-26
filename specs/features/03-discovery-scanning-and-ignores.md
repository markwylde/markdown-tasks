# Discovery, scanning, and ignores

## Targets

For a file target, validate that it is a regular file with one of these
case-insensitive extensions:

- `.md`
- `.markdown`
- `.mdown`
- `.mkd`

For a directory target, recursively scan supported files. Sort directory entries
and final documents case-insensitively with numeric segments handled naturally so
output is deterministic across platforms.

Symlinked directories are not followed. A symlinked file may be read only when it
is the explicit target; recursive scans skip symlinks to prevent cycles and
escaping the selected root.

## Default ignored directories

Match Terminay's practical defaults:

```text
.git
.hg
.svn
node_modules
bower_components
dist
build
out
.next
.nuxt
.cache
coverage
target
vendor
.venv
venv
__pycache__
```

`--no-default-ignore` disables this list. Each `--ignore` adds a directory name or
root-relative directory path. Explicitly targeting an otherwise ignored directory
scans that directory because it is the root.

## Scan result

A scan reports:

- supported Markdown files inspected;
- directories inspected/watched;
- ignored directories encountered;
- files containing at least one task;
- files complete;
- total, completed, and remaining tasks;
- warnings with paths and concise causes.

Documents without tasks count as scanned Markdown files but do not appear as task
documents and do not affect “files complete.”

## Error handling

- Missing, unsupported, or wholly unreadable explicit targets are fatal.
- Failure to read one descendant is a warning; continue scanning siblings.
- Invalid UTF-8 is decoded lossily with a warning so useful surrounding tasks can
  still appear.
- Individual files have a reasonable size guard to avoid accidental memory
  exhaustion. The initial implementation should use 16 MiB and report skipped
  larger files as warnings.
- Never panic on filesystem races between directory enumeration and file reading.

## Performance budget

On a warm local filesystem, a tree of 10,000 files with 1,000 Markdown files
should produce its initial snapshot in under one second on a typical development
machine. Parsing and traversal must remain off the TUI input/render path.

Avoid premature incremental indexing in v1. A full rescan after a debounced event
is acceptable if it meets the budget and keeps correctness simple.
