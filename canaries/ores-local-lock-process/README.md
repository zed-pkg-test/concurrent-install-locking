# ORES local-lock process canary

Independent consumer evidence for the portable local filesystem backend in `ORESoftware/ores-locks-and-leases`.

The canary intentionally depends on the Rust crate with `default-features = false`, so a normal single-host install lock has no Redis, Fiducia, Cloudflare Durable Objects, PostgreSQL, or other distributed-lock dependency.

Each matrix run creates a clean temporary user-home analogue and uses a rendezvous under `$HOME/.zpkg/locks/install.lock`. Five waves each start 12 real OS processes behind a barrier and require exactly one winner plus 11 contenders. After release, the lock rendezvous must be absent while the caller-owned `.zpkg` parent directories and unrelated home sentinel remain intact.

The workflow source-pins the exact production commit under test and runs on Ubuntu, macOS, and Windows. A green run is evidence only for that pinned production revision; it must not be reused as proof for a different head.
