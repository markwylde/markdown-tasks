# Group 04 — Non-interactive report

Depends on: Group 03

Outcome: `mdt PATH` prints a complete, deterministic, readable report and exits.

## Projection and rendering

- [ ] Define a plain-report projection independent of terminal width.
- [ ] Render aggregate percent, done, remaining, and files-complete summary.
- [ ] Render scanned Markdown/directory counts and ignored count when nonzero.
- [ ] Render warnings with normalized relative paths and concise causes.
- [ ] Render every task document with per-file progress.
- [ ] Render heading hierarchy, section progress, task indentation, and source order.
- [ ] Render `[x]` and `[ ]` without implying that output is interactive.
- [ ] Handle empty-label tasks as `(untitled task)`.
- [ ] Render deliberate no-files, no-tasks, and all-complete states.

## Output behavior

- [ ] Implement `auto`, `always`, and `never` color behavior.
- [ ] Ensure redirected default output contains no ANSI or cursor-control bytes.
- [ ] Avoid truncating labels and paths in redirected output.
- [ ] Emit exactly one final newline and no debug output.
- [ ] Keep partial warnings compatible with exit code 0.

## Verification

- [ ] Add golden fixtures for mixed progress, single file, root tasks, nested
      sections/tasks, warnings, no tasks, all complete, Unicode, and no color.
- [ ] Add a process test proving the command exits without reading stdin.
- [ ] Add platform-path normalization tests.
- [ ] Manually verify `mdt specs/tasks` against the feature-plan directory.
