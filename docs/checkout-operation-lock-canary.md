# Checkout-local mutation ownership release canary

This repository provides an independent locking-focused release gate for the
complete remaining DEN-2038 product candidate. It imports the exact black-box
harness from `zed-pkg-test/zed-pkg-e2e` and executes it against the exact
three-file `zed-pkg/zed-cli` candidate.

## Immutable source graph

```text
zed-cli        28c68124e87301f562a8d423410943cf2de61064
zed-pkg-e2e    09fe56b65e77f9bcb6c07bab5717aa3bcd0a86c5
zed-interfaces d1bf8ef7c88a75292cbe8d697bfd269f30d62e2b
zed-lock       a0dc78d385bc3ab553d3027b427f5f1428239c9c
```

The product branch was semantically composed with reviewed `main` through true
merge commits, preserving independent GitOps and DEN-3018 publish-ignore work.
The final feature delta is limited to `src/project_lock.rs`,
`src/git_submodules/cli.rs`, and `src/main.rs`; temporary finalizer artifacts
were removed before certification.

The exact E2E commit includes the primary DEN-2038 harness and a static source
ratchet requiring cooperative installation to call
`managed_install::install(&project, ...)`, never the caller's nested `&cwd`.

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
- one selected-superproject guard spanning nested cooperative
  `install --git-submodules`, from recovery and Git sync through dependency
  resolution, materialization, adapter wiring, and final lock publication.

## Nineteen process assertions

The imported harness proves:

1. takeover owns the project lock before Git transport;
2. a symlink-alias frozen install blocks;
3. normal release publishes complete adopted state before the waiter succeeds;
4. an ordinary install blocks behind a second takeover;
5. owner-process termination releases the descriptor lock;
6. pre-mutation termination preserves manifest bytes and leaves no staging;
7. nested takeover owns the superproject lock;
8. no nested takeover lock identity is created;
9. a root frozen install serializes behind nested takeover;
10. cooperative install launched from `packages/client/src` owns the root lock
    before Git sync;
11. nested cooperative invocation creates no nested lock, manifest, lockfile, or
    staging identity;
12. a different-home root frozen install blocks through Git sync and the full
    install/finalizer lifecycle;
13. the waiter succeeds only after complete root lockfile and child state
    publication;
14. different-home recovery blocks behind checkout ownership;
15. destination and backup bytes remain exact while blocked;
16. release restores exact bytes and removes staging;
17. the recovered process completes frozen installation;
18. modular takeover recovers before `git submodule sync`; and
19. exact backup bytes are restored before verification, transport, or adoption.

## Workflow policy

The Ubuntu 24.04 and macOS 15 matrix uses only public, immutable sources and
commit-pinned Actions. It has `contents: read`, does not persist checkout
credentials, statically rejects cooperative `&cwd`, disables Python bytecode
writes, keeps compilation caches in runner temporary storage, denies Clippy
warnings on Linux, builds the exact release binary, rejects dirty source trees
without cleanup/reset, and retains only bounded process evidence for 14 days.

No user PAT, GitHub App secret, Linear token, Cloudflare token, R2 credential,
public package registry, Docker daemon, or persistent namespace participates.

The primary promotion gate remains `zed-pkg-test/zed-pkg-e2e#139`. This canary is
additive evidence and cannot turn a failing primary gate into a passing release.
Linear: DEN-2038.
