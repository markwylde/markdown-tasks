# Non-interactive report

## Purpose

Running `mdt PATH` must be useful in a normal shell, CI log, redirected file, or
agent transcript. It scans once, renders one complete snapshot, and closes without
reading stdin or starting a watcher.

## Shape

The default report has three parts:

1. one aggregate summary line;
2. optional scan metadata and warnings;
3. every matching document, section, and task in an expanded tree.

Example (exact spacing may adapt, wording should remain stable):

```text
75% complete  |  6 done  |  2 remaining  |  1/2 files complete
2 markdown files scanned in 3 directories

plan.md  3/4  75%
  Project plan  3/4
    Phase 1  2/2
      [x] Scaffold the CLI
      [x] Parse checkboxes
    Phase 2  1/2
      [x] Add static output
      [ ] Add the TUI

nested/release.md  3/4  75%
  ...
```

Tasks retain their source nesting beneath the owning section. Completed tasks use
`[x]`; remaining tasks use `[ ]`. The report is read-only.

## Determinism

- Documents use normalized `/` separators in displayed relative paths.
- Sections use source hierarchy unless a future explicit CLI sort option says
  otherwise.
- Tasks remain in source order.
- No timestamps, spinners, cursor control, or terminal-width-dependent omission.
- When stdout is not a TTY, default output contains no ANSI escapes.
- Long labels are not truncated in redirected output.

## States

- No supported Markdown files: print a concise scan result and exit 0 for a
  directory, but reject an unsupported explicit file as invalid input.
- Supported files but no tasks: print `No tasks found.` plus scan metadata, exit 0.
- Partial warnings: render a `Warnings:` block after scan metadata and continue.
- All tasks complete: retain the normal tree and clearly state `100% complete`.

## Snapshot testing

Golden tests cover a mixed directory, a single file, no tasks, all complete,
Unicode, warnings, `--color never`, and normalized path separators. Tests must
strip no output before comparison except a final platform newline conversion.
