# Test plan

- Verify five concurrent installs, per-artifact locks, killed-owner recovery, and corruption resistance across the supported happy-path states and canonical fixtures.
- Verify five concurrent installs, per-artifact locks, killed-owner recovery, and corruption resistance under retries, interruption, concurrency, offline operation, or partial failure.
- Verify five concurrent installs, per-artifact locks, killed-owner recovery, and corruption resistance preserves authorization, idempotency, integrity, observability, and actionable failure classification.

## Classification

- product regression
- blocked dependency
- harness regression
