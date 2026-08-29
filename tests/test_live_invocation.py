from __future__ import annotations

import os
import shlex
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUN_CHAOS = ROOT / "scripts" / "run-chaos-suite.sh"


def executable(path: Path, body: str) -> Path:
    path.write_text("#!/usr/bin/env bash\nset -euo pipefail\n" + body)
    path.chmod(0o755)
    return path


def base_environment(bin_dir: Path) -> dict[str, str]:
    environment = os.environ.copy()
    environment["PATH"] = f"{bin_dir}{os.pathsep}{environment['PATH']}"
    environment.pop("CHAOS_TEST_COMMAND", None)
    return environment


def test_default_live_invocation_runs_the_real_crash_recovery_case(tmp_path: Path) -> None:
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    command_log = tmp_path / "cargo-command.txt"
    executable(
        bin_dir / "cargo",
        'printf "%s|%s\\n" "${ZED_LOCK_E2E_STRESS:-}" "$*" > "$CHAOS_COMMAND_LOG"\n',
    )
    environment = base_environment(bin_dir)
    environment["CHAOS_COMMAND_LOG"] = str(command_log)

    subprocess.run([RUN_CHAOS], cwd=ROOT, env=environment, check=True)

    assert command_log.read_text().strip() == (
        "1|test --release --test process_locking "
        "killed_owner_releases_the_lock_and_preserves_the_rendezvous_file "
        "-- --exact --nocapture"
    )


def test_explicit_network_chaos_command_owns_the_proxy_lifecycle(tmp_path: Path) -> None:
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    docker_log = tmp_path / "docker-commands.txt"
    command_log = tmp_path / "custom-command.txt"
    executable(
        bin_dir / "docker",
        'printf "%s\\n" "$*" >> "$DOCKER_COMMAND_LOG"\n',
    )
    environment = base_environment(bin_dir)
    environment["DOCKER_COMMAND_LOG"] = str(docker_log)
    environment["CHAOS_TEST_COMMAND"] = (
        f"printf 'custom-ok\\n' > {shlex.quote(str(command_log))}"
    )

    subprocess.run([RUN_CHAOS], cwd=ROOT, env=environment, check=True)

    assert command_log.read_text() == "custom-ok\n"
    assert docker_log.read_text().splitlines() == [
        "compose -f docker-compose.chaos.yml up -d",
        "compose -f docker-compose.chaos.yml down -v",
    ]
