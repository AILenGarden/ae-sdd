from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path


TOOLS_DIR = Path(__file__).resolve().parent.parent


def _spawn_acquire(state_path: Path, barrier: Path, owner: str) -> subprocess.Popen[str]:
    script = r"""
import json
import sys
import time
from pathlib import Path

from lib.state_store import LeaseOwner, StateStore, StateStoreError

state_path = Path(sys.argv[1])
barrier = Path(sys.argv[2])
owner_name = sys.argv[3]
while not barrier.exists():
    time.sleep(0.005)
try:
    lease = StateStore(state_path).acquire_lease(
        LeaseOwner(owner_name, f"session-{owner_name}", "subprocess-host", 1),
        ttl_seconds=300,
        idempotency_key=f"acquire-{owner_name}",
    )
    print(json.dumps({"ok": True, "token": lease.fencing_token, "owner": owner_name}))
except StateStoreError as exc:
    print(json.dumps({"ok": False, "code": exc.code, "owner": owner_name}))
"""
    env = os.environ.copy()
    env["PYTHONPATH"] = str(TOOLS_DIR)
    env["PYTHONIOENCODING"] = "utf-8"
    return subprocess.Popen(
        [sys.executable, "-c", script, str(state_path), str(barrier), owner],
        cwd=str(TOOLS_DIR.parent),
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )


def _collect(proc: subprocess.Popen[str]) -> dict:
    stdout, stderr = proc.communicate(timeout=20)
    assert proc.returncode == 0, stderr
    return json.loads(stdout)


def _write_state(path: Path) -> None:
    path.parent.mkdir(parents=True)
    path.write_text(
        json.dumps({"version": "2", "phase": "initialized", "history": []}),
        encoding="utf-8",
    )


def test_two_processes_competing_for_same_work_item_have_one_winner(tmp_path: Path) -> None:
    state_path = tmp_path / "work-item" / "state.json"
    barrier = tmp_path / "start"
    _write_state(state_path)
    first = _spawn_acquire(state_path, barrier, "A")
    second = _spawn_acquire(state_path, barrier, "B")

    barrier.touch()
    results = [_collect(first), _collect(second)]

    assert sum(1 for result in results if result["ok"]) == 1
    loser = next(result for result in results if not result["ok"])
    assert loser["code"] == "LEASE_CONFLICT"
    lease = json.loads((state_path.parent / "state.lease.json").read_text(encoding="utf-8"))
    assert lease["status"] == "active"
    assert lease["fencingToken"] == 1


def test_different_work_items_do_not_block_each_other(tmp_path: Path) -> None:
    first_state = tmp_path / "work-item-a" / "state.json"
    second_state = tmp_path / "work-item-b" / "state.json"
    barrier = tmp_path / "start"
    _write_state(first_state)
    _write_state(second_state)
    first = _spawn_acquire(first_state, barrier, "A")
    second = _spawn_acquire(second_state, barrier, "B")

    barrier.touch()
    results = [_collect(first), _collect(second)]

    assert all(result["ok"] for result in results)
    assert {result["token"] for result in results} == {1}
