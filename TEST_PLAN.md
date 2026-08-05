# Test plan

- Verify concurrent recursive/store operations serialize on one logical lock identity while unrelated identities retain useful concurrency.
- Verify evented waiter threads issue one blocking native acquisition request, survive repeated timeout observation, and never leak a late guard after cancellation.
- Verify killed-owner recovery, panic/process exit behavior, and stable lock-file rendezvous semantics.
- Verify deterministic lock-class/path ordering, canonical identity deduplication, and Unix symlink alias contention.
- Verify thread and process contention preserves protected counters without lost updates or overlapping critical sections on Linux, macOS, and Windows.
- Verify larger scheduled contention soaks remain bounded, observable, network-free on the local path, and actionable when they fail.

## Automated tiers

### Pull request and main branch

- Ubuntu, macOS, and Windows Rust conformance matrix.
- Formatting and strict Clippy.
- Threaded hot-key/cold-key isolation.
- Timeout, cancellation, ordering, alias, process handoff, owner-death, and protected-counter cases.

### Scheduled and manual

- The normal three-platform matrix.
- Larger Ubuntu release-mode thread and process contention soak through `ZED_LOCK_E2E_STRESS=1`.
- Existing live dependency/bootstrap and chaos readiness paths.

## Classification

- **product regression** — the production `zed-lock` behavior violates a locking invariant after dependency resolution succeeds;
- **blocked dependency** — the production source, credential, runner capability, or required infrastructure is unavailable;
- **harness regression** — the test repository, fixture, workflow, or evidence-generation code is invalid.
