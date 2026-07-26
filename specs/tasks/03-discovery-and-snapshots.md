# Group 03 — Discovery and snapshots

Depends on: Groups 01–02

Outcome: a file or directory target produces a deterministic workspace snapshot
with accurate scan metadata and recoverable warnings.

## Target and traversal

- [x] Accept a regular supported Markdown file or directory as the scan root.
- [x] Implement recursive traversal for `.md`, `.markdown`, `.mdown`, and `.mkd`
      case-insensitively.
- [x] Skip directory symlinks and recursive file symlinks.
- [x] Apply the complete built-in ignored-directory list.
- [x] Apply repeatable name and root-relative path ignores.
- [x] Implement `--no-default-ignore` while retaining explicit ignores.
- [x] Normalize displayed relative paths to `/` without corrupting native paths.
- [x] Sort traversal results deterministically and numeric-aware.

## Snapshot construction

- [x] Read and parse supported files outside UI code.
- [x] Exclude zero-task documents from task-document/file-complete counts while
      retaining scanned-Markdown counts.
- [x] Combine all document stats into aggregate stats.
- [x] Count scanned directories, ignored directories, Markdown files, task files,
      and complete task files.
- [x] Enforce the 16 MiB per-file guard.
- [x] Decode invalid UTF-8 lossily and attach a path-specific warning.
- [x] Continue after descendant read/enumeration races and permission failures.
- [x] Treat an unreadable explicit target as fatal.

## Verification

- [x] Use temporary-directory tests for extension case, nested files, ignores,
      explicit ignored roots, symlinks, unreadable/missing files, and large files.
- [x] Test deterministic document order on every supported platform.
- [x] Build a performance fixture approximating 10,000 files/1,000 Markdown files
      and record the baseline without making timing tests flaky in CI.
