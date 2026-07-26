# Live watch and reload

## Scope

Filesystem watching runs only in `--tui` mode. Watch the selected file or the
selected directory recursively. New, removed, renamed, and modified supported
Markdown files and directories must trigger refresh. Events entirely inside
ignored directories must not.

Use the cross-platform `notify` crate through a small internal abstraction so
watch behavior can be tested with synthetic events.

## Event pipeline

```text
notify callback -> bounded channel -> classify/coalesce -> debounce -> scan worker
                                                        -> snapshot message -> UI
```

- Never scan or parse inside the native watcher callback.
- Coalesce bursts with a 200 ms quiet-period debounce.
- At most one scan runs at once.
- If an event arrives during a scan, schedule exactly one additional scan after it.
- Watcher overflow/rescan signals force a full scan.
- The UI event loop remains responsive while scanning.

The debounce duration is an internal constant in v1 and may become configurable
only if real-world evidence warrants it.

## Snapshot swap

On successful refresh:

- atomically replace the current snapshot;
- reconcile selection and collapse state by stable keys;
- update watched/scanned counts;
- show `updated just now`, then return to `up to date`;
- redraw only as needed.

On failed refresh:

- keep the last good snapshot;
- show a persistent, concise error status;
- continue watching and allow `r` to retry.

If the explicit root is deleted, keep the last snapshot, report `target missing`,
and continue watching its nearest existing parent when the platform supports it so
recreation can recover without restarting.

## Manual refresh

`r` bypasses the debounce delay but still obeys the single-flight rule. While a
scan runs, show a spinner or `refreshing…`. Repeated `r` presses collapse into one
pending refresh.

## Watcher portability

Native event delivery is best-effort. Network filesystems may not emit reliable
events. The help/status text should suggest manual `r` after a watcher error.
Polling fallback is not required for v1, but the watcher abstraction should allow
one later.

## Shutdown

Quitting cancels or joins the scan worker, drops the watcher, closes channels, and
restores the terminal without hanging. Automated tests must cover quit during an
active refresh and late worker messages after shutdown begins.
