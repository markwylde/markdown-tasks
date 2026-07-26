# Group 03 — Discovery and snapshots

Depends on: Groups 01–02

Outcome: a file or directory target produces a deterministic workspace snapshot
with accurate scan metadata and recoverable warnings.

## Target and traversal

- [ ] Accept a regular supported Markdown file or directory as the scan root.
- [ ] Implement recursive traversal for `.md`, `.markdown`, `.mdown`, and `.mkd`
      case-insensitively.
- [ ] Skip directory symlinks and recursive file symlinks.
- [ ] Apply the complete built-in ignored-directory list.
- [ ] Apply repeatable name and root-relative path ignores.
- [ ] Implement `--no-default-ignore` while retaining explicit ignores.
- [ ] Normalize displayed relative paths to `/` without corrupting native paths.
- [ ] Sort traversal results deterministically and numeric-aware.

## Snapshot construction

- [ ] Read and parse supported files outside UI code.
- [ ] Exclude zero-task documents from task-document/file-complete counts while
      retaining scanned-Markdown counts.
- [ ] Combine all document stats into aggregate stats.
- [ ] Count scanned directories, ignored directories, Markdown files, task files,
      and complete task files.
- [ ] Enforce the 16 MiB per-file guard.
- [ ] Decode invalid UTF-8 lossily and attach a path-specific warning.
- [ ] Continue after descendant read/enumeration races and permission failures.
- [ ] Treat an unreadable explicit target as fatal.

## Verification

- [ ] Use temporary-directory tests for extension case, nested files, ignores,
      explicit ignored roots, symlinks, unreadable/missing files, and large files.
- [ ] Test deterministic document order on every supported platform.
- [ ] Build a performance fixture approximating 10,000 files/1,000 Markdown files
      and record the baseline without making timing tests flaky in CI.
