#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

if [[ -z "${CHAOS_TEST_COMMAND:-}" ]]; then
  command -v cargo >/dev/null || { echo "blocked: cargo unavailable"; exit 78; }
  ZED_LOCK_E2E_STRESS=1 cargo test --release --test process_locking \
    killed_owner_releases_the_lock_and_preserves_the_rendezvous_file \
    -- --exact --nocapture
  exit 0
fi

command -v docker >/dev/null || { echo "blocked: docker unavailable"; exit 78; }
docker compose -f docker-compose.chaos.yml up -d
trap 'docker compose -f docker-compose.chaos.yml down -v' EXIT
bash -lc "$CHAOS_TEST_COMMAND"
