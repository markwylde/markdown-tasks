# Group 08 — Live watch and reload

Depends on: Groups 03, 05, and 06; Group 07 for final Kanban reconciliation

Outcome: the interactive explorer automatically reloads after relevant filesystem
changes without freezing or losing useful UI state.

## Watch infrastructure

- [ ] Define a watcher trait and production `notify` implementation.
- [ ] Watch the explicit file or directory recursively only in TUI mode.
- [ ] Filter irrelevant and ignored-path events.
- [ ] Handle create, modify, remove, rename, overflow, and rescan indications.
- [ ] Send watcher callbacks through a bounded channel without scanning in callback.
- [ ] Add a 200 ms quiet-period debounce using an injectable clock.
- [ ] Implement a single-flight scan worker with one coalesced pending refresh.
- [ ] Ensure `r` requests immediate work while respecting single-flight behavior.

## Refresh behavior

- [ ] Swap in only complete successful snapshots.
- [ ] Keep the last good snapshot on failure and show a persistent status.
- [ ] Reconcile list rows, Kanban cards, selection, collapse, filter, search, sort,
      grouping, and viewport positions after refresh.
- [ ] Show refreshing, updated, up-to-date, watcher-error, and target-missing states.
- [ ] Recover when a missing target is recreated where the platform watcher permits.
- [ ] Keep accepting input and resize events during scans.

## Shutdown

- [ ] Stop accepting refresh requests once shutdown starts.
- [ ] Drop watcher resources, cancel/join workers, and close channels promptly.
- [ ] Ignore late worker messages safely.
- [ ] Restore the terminal when quitting during an active scan.

## Verification

- [ ] Drive synthetic watcher/debounce/single-flight tests without real sleeps.
- [ ] Integration-test create, modify, delete, and rename with temporary directories.
- [ ] Test an event arriving during a scan produces exactly one follow-up scan.
- [ ] Test refresh failure followed by recovery.
- [ ] Test stable selection/collapse after changes and graceful quit mid-refresh.
- [ ] Manually verify native watcher behavior on macOS, Linux, and Windows.
