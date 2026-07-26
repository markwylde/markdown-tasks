# Performance baseline

The deterministic large-fixture test creates 10,000 files across 10 directories,
including 1,000 Markdown task files, then discovers, reads, parses, sorts, and
aggregates the resulting snapshot.

Baseline recorded on 26 July 2026 on the development macOS machine with an
already-built release test binary:

```text
real 0.92s
user 0.11s
sys  0.78s
test body 0.78s
```

Run the same non-flaky workload with:

```sh
cargo test --release \
  snapshot::tests::builds_a_reasonable_large_fixture_without_a_timing_assertion \
  -- --exact
```

The test deliberately asserts correctness and fixture size rather than elapsed
time. Profiling did not reveal a code bottleneck requiring optimization; most of
this local workload is temporary-file creation and filesystem metadata I/O.
