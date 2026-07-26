# CLI contract

## Synopsis

```text
mdt [OPTIONS] [PATH]

Arguments:
  [PATH]  Markdown file or directory to inspect [default: .]

Options:
      --tui                  Open the interactive live task explorer
      --ignore <PATTERN>     Add an ignored directory name or root-relative path
      --no-default-ignore    Disable the built-in ignored-directory list
      --color <WHEN>         auto, always, or never [default: auto]
  -h, --help                 Print help
  -V, --version              Print version
```

`--ignore` is repeatable. A pattern containing `/` is matched against a normalized
path relative to the target root; a pattern without `/` matches a directory name
at any depth. v1 does not expose glob syntax, avoiding platform-dependent surprises.

## Mode selection

- Without `--tui`, scan once, write the report to stdout, and exit.
- With `--tui`, require an interactive terminal, enter the alternate screen, scan,
  render, watch, and remain open until the user quits.
- `PATH` may be a supported Markdown file or a directory.
- Resolve `PATH` to a normalized absolute path internally while displaying the
  user-supplied path and root-relative document paths where possible.

No environment variable silently enables the TUI.

## Output streams

- Successful non-interactive reports go to stdout.
- Help and version use conventional CLI behavior.
- Fatal diagnostics go to stderr.
- Non-fatal scan warnings appear in the report/TUI status but do not corrupt
  machine-consumable stdout with unrelated logging.
- TUI logs, if enabled for development, go to a file rather than stdout/stderr.

## Color and symbols

- `auto`: emit styling only when stdout is a terminal and color is supported.
- `always`: emit ANSI styling even when redirected.
- `never`: emit no ANSI escapes.
- Honor `NO_COLOR` unless `--color always` is explicitly supplied.
- The plain report must remain readable with ASCII-only terminal capabilities.
- The TUI may use Unicode glyphs, but must have safe fallbacks for borders,
  checkmarks, ellipses, and progress bars.

## Exit status

| Code | Meaning |
| ---: | --- |
| 0 | Scan/report completed, including the valid “no tasks found” case |
| 1 | Runtime failure after arguments were accepted, such as unreadable target |
| 2 | Invalid arguments, unsupported file extension, or `--tui` without a TTY |

Partial directory scan errors do not force a nonzero exit when at least one
document was scanned successfully. They are rendered as warnings.

## Terminal lifecycle

The interactive path must restore raw mode, cursor visibility, mouse mode, and the
alternate screen on normal exit, error, panic, SIGINT, and SIGTERM where the
platform permits. Fatal TUI errors are printed only after restoration.

## Acceptance examples

```sh
mdt
mdt specs/tasks
mdt README.md
mdt --color never specs/tasks > task-report.txt
mdt --ignore fixtures --ignore generated/docs specs/tasks
mdt --tui specs/tasks
```
