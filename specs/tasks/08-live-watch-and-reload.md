# Group 08 — Live watch and reload

Depends on: Groups 03, 05, and 06; Group 07 for final Kanban reconciliation

Outcome: the interactive explorer automatically reloads after relevant filesystem
changes without freezing or losing useful UI state.

## Watch infrastructure

- [x] Define a watcher trait and production `notify` implementation.
- [x] Watch the explicit file or directory recursively only in TUI mode.
- [x] Filter irrelevant and ignored-path events.
- [x] Handle create, modify, remove, rename, overflow, and rescan indications.
- [x] Send watcher callbacks through a bounded channel without scanning in callback.
- [x] Add a 200 ms quiet-period debounce using an injectable clock.
- [x] Implement a single-flight scan worker with one coalesced pending refresh.
- [x] Ensure `r` requests immediate work while respecting single-flight behavior.

## Refresh behavior

- [x] Swap in only complete successful snapshots.
- [x] Keep the last good snapshot on failure and show a persistent status.
- [x] Reconcile list rows, Kanban cards, selection, collapse, filter, search, sort,
      grouping, and viewport positions after refresh.
- [x] Show refreshing, updated, up-to-date, watcher-error, and target-missing states.
- [x] Recover when a missing target is recreated where the platform watcher permits.
- [x] Keep accepting input and resize events during scans.

## Shutdown

- [x] Stop accepting refresh requests once shutdown starts.
- [x] Drop watcher resources, cancel/join workers, and close channels promptly.
- [x] Ignore late worker messages safely.
- [x] Restore the terminal when quitting during an active scan.

## Verification

- [x] Drive synthetic watcher/debounce/single-flight tests without real sleeps.
- [x] Integration-test create, modify, delete, and rename with temporary directories.
- [x] Test an event arriving during a scan produces exactly one follow-up scan.
- [x] Test refresh failure followed by recovery.
- [x] Test stable selection/collapse after changes and graceful quit mid-refresh.
