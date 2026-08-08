# Checkout-local mutation ownership release canary

This repository provides an independent locking-focused release gate for the
complete remaining DEN-2038 product candidate. It imports the exact black-box
harness from `zed-pkg-test/zed-pkg-e2e` and executes it against a
current-main-integrated `zed-pkg/zed-cli` commit.

## Immutable source graph

```text
zed-cli        2b5897f6a5b5cf33fee7a5934a60d7aa3e70e0a0
zed-pkg-e2e    4be9cfc93805ab7a6d9ac2e8e2ccefd1d6ee5f29
zed-interfaces d1bf8ef7c88a75292cbe8d697bfd269f30d62e2b
zed-lock       a0dc78d385bc3ab553d3027b427f5f1428239c9c
```

The product branch is zero commits behind the reviewed current `main`. Earlier
integration used true merge commits to preserve both the complete DEN-2038
checkout-lock/recovery changes and current main's independent GitOps and
DEN-3018 publish-ignore work. The final feature delta remains limited to three
product files.

The exact E2E commit includes the primary DEN-2038 harness plus disjoint current-
main test-org integrations. The six certification files are the only PR-visible
feature delta.

## Product boundary

All checkout-tree mutations use one canonical `.zed/operation.lock` identity.
The reviewed product provides:

- Git-submodule takeover ownership shared with ordinary lifecycle facades;
- superproject discovery before nested takeover lock acquisition;
- same-thread RAII reentrancy through shared `Rc<OwnedLock>` descriptor
  ownership;
- a weak thread-local lookup that never extends lock lifetime;
- a naturally `!Send + !Sync` public guard;
- descriptor ownership that survives outer-before-inner guard drops;
- ordinary transaction recovery serialized across different Zed homes;
- modular takeover recovery before any `.gitmodules` read or Git transport; and
- one superproject guard spanning cooperative `install --git-submodules`, from
  recovery and Git sync through dependency resolution, materialization, adapter
  wiring, and final lock publication.

## Eighteen process assertions

The imported harness proves:

1. takeover owns the project lock before Git transport;
2. a symlink-alias frozen install blocks;
3. normal release publishes complete adopted state before the waiter succeeds;
4. an ordinary install blocks behind a second takeover;
5. owner-process termination releases the descriptor lock;
6. pre-mutation termination preserves manifest bytes and leaves no staging;
7. nested takeover owns the superproject lock;
8. no nested lock identity is created;
9. a root frozen install serializes behind nested takeover;
10. cooperative `install --git-submodules` owns the lock before Git sync;
11. a different-home frozen install blocks through Git sync and installation;
12. the waiter succeeds only after complete lockfile and child state publication;
13. different-home recovery blocks behind checkout ownership;
14. destination and backup bytes remain exact while blocked;
15. release restores exact bytes and removes staging;
16. the recovered process completes frozen installation;
17. modular takeover recovers before `git submodule sync`; and
18. exact backup bytes are restored before verification, transport, or adoption.

## Workflow policy

The Ubuntu 24.04 and macOS 15 matrix uses only public, immutable sources and
commit-pinned Actions. It has `contents: read`, does not persist checkout
credentials, disables Python bytecode writes, keeps compilation caches in runner
temporary storage, denies Clippy warnings on Linux, builds the exact release
binary, rejects dirty source trees without cleanup/reset, and retains only
bounded process evidence for 14 days.

No user PAT, GitHub App secret, Linear token, Cloudflare token, R2 credential,
public package registry, Docker daemon, or persistent namespace participates.

The primary promotion gate remains `zed-pkg-test/zed-pkg-e2e#139`. This canary is
additive evidence and cannot turn a failing primary gate into a passing release.
Linear: DEN-2038.
