# concurrent-install-locking

Independent **chaos/fault-injection and cross-platform locking** harness in `zed-pkg-test` for `zed-pkg`.

**Readiness:** `ready`  
**Primary dependency strategy:** `matrix`  
**Scheduled live cadence:** `17 5 * * *` UTC  
**Scheduled locking cadence:** `43 5 * * *` UTC  
**Live infrastructure:** multi-process runner

## Upstream repositories

- `zed-pkg/zed-lock`
- `zed-pkg/zed-cli`
- `zed-pkg/zed-interfaces`

The Rust acceptance harness resolves the production `zed-lock` package directly from the standalone `zed-pkg/zed-lock` repository at immutable commit `6ddc734e786b54848621fe60017cc2ea47d4165d`. This is the merge of the DEN-3854 formal waiter/ownership lifecycle and keeps the test organization independent from both the production CLI workspace and mutable branches while certifying the exact standalone source.

## Acceptance objectives

1. Verify concurrent installs, per-artifact locks, killed-owner recovery, and corruption resistance across supported happy-path states and canonical fixtures.
2. Verify blocking kernel wake-up and evented completion under retries, timeout observation, cancellation, interruption, concurrency, and partial failure without converting acquisition into userspace polling.
3. Verify independent lock identities retain concurrency while one hot identity has many blocked thread or process waiters.
4. Verify canonical aliases, deterministic multi-lock ordering, bounded waiter resources, crash cleanup, and stable lock-file rendezvous semantics.
5. Verify locking preserves idempotency, integrity, observability, actionable failure classification, and cross-platform behavior on Linux, macOS, and Windows.

## Locking tests

The cross-platform Rust suite covers:

- one cold lock completing while twelve waiter threads are blocked on another lock;
- sharded thread contention with protected file counters;
- repeated caller timeouts producing one native acquisition event rather than a retry loop;
- deadline timeout producing exactly one terminal reason while a later native
  grant is immediately released;
- cancellation followed by late native acquisition and immediate RAII release;
- deterministic lock-class/path ordering and duplicate canonical identity rejection;
- Unix symlink aliases sharing one lock domain;
- six independent processes receiving exclusive handoffs without FIFO assumptions;
- forced owner termination and successor acquisition while preserving the lock file;
- unrelated process lock identities progressing concurrently; and
- multi-process protected-counter tests, with larger thread and process counts in scheduled soak mode.

```bash
cargo test --all-targets -- --nocapture
ZED_LOCK_E2E_STRESS=1 cargo test --release --all-targets -- --nocapture
```

`.github/workflows/locking-matrix.yml` runs the normal suite on Ubuntu, macOS, and Windows for pull requests and main-branch changes. Scheduled and manually dispatched runs also execute the larger Ubuntu release-mode soak.

## Dependency paths

This repository also tests the CLI upstream through independent installation paths:

1. `./scripts/bootstrap-upstream.sh git-submodule`
2. `./scripts/bootstrap-upstream.sh zed`
3. `./scripts/bootstrap-upstream.sh native-package`

The publisher materializes a real Git submodule when authenticated access is available. Zed and native package coordinates are recorded in `dependency-contract.yaml`; missing unpublished packages are reported as blocked readiness rather than silently skipped.

## Check tiers

```bash
python3 -m pip install -e '.[test]'
pytest -q
./scripts/readiness.py --offline
./scripts/run-live.sh
cargo test --all-targets -- --nocapture
```

With no extra configuration, `run-live.sh` executes the release-mode
multi-process crash-recovery case: it kills the current lock owner and requires
a successor to acquire the same stable rendezvous file. Set
`CHAOS_TEST_COMMAND` only for an additional network-fault command; that explicit
mode starts the Toxiproxy service from `docker-compose.chaos.yml` and requires
Docker. The built-in scheduled crash test does not start an unused proxy.

Pull requests validate the harness, deterministic contract fixtures, and the normal three-platform locking matrix. Secret-, service-, emulator-, desktop-, database-, provider-, chaos-, scale-, and extended-soak checks run by schedule or manual dispatch.

A live result must be classified as one of:

- **product regression** — a behavioral invariant fails after dependencies are ready;
- **blocked dependency** — an upstream, credential, package, emulator, provider sandbox, or deployment is unavailable;
- **harness regression** — generated metadata, fixtures, workflow, or runner setup is invalid.

Managed by `github-test-org-factory/1.0.0` with repository-specific locking coverage layered on top.
