# Group 04 — Non-interactive report

Depends on: Group 03

Outcome: `mdt PATH` prints a complete, deterministic, readable report and exits.

## Projection and rendering

- [x] Define a plain-report projection independent of terminal width.
- [x] Render aggregate percent, done, remaining, and files-complete summary.
- [x] Render scanned Markdown/directory counts and ignored count when nonzero.
- [x] Render warnings with normalized relative paths and concise causes.
- [x] Render every task document with per-file progress.
- [x] Render heading hierarchy, section progress, task indentation, and source order.
- [x] Render `[x]` and `[ ]` without implying that output is interactive.
- [x] Handle empty-label tasks as `(untitled task)`.
- [x] Render deliberate no-files, no-tasks, and all-complete states.

## Output behavior

- [x] Implement `auto`, `always`, and `never` color behavior.
- [x] Ensure redirected default output contains no ANSI or cursor-control bytes.
- [x] Avoid truncating labels and paths in redirected output.
- [x] Emit exactly one final newline and no debug output.
- [x] Keep partial warnings compatible with exit code 0.

## Verification

- [x] Add golden fixtures for mixed progress, single file, root tasks, nested
      sections/tasks, warnings, no tasks, all complete, Unicode, and no color.
- [x] Add a process test proving the command exits without reading stdin.
- [x] Add platform-path normalization tests.
- [x] Manually verify `mdt specs/tasks` against the feature-plan directory.
