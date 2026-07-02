from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from build_harness import (  # noqa: E402
    ADAPTER_VERSION,
    cleanup_old_bak,
    get_commit_hash,
    get_tree_hash,
    read_adapter_lock,
    source_input_hash,
    template_hash,
)


def _source_input_hash(repo: Path) -> str:
    return source_input_hash(
        repo,
        repo / "scripts" / "templates" / "agent.md.template",
        repo / "scripts" / "templates" / "README.md.template",
    )


def _template_hash(repo: Path) -> str:
    return template_hash(repo / "scripts" / "templates" / "agent.md.template")


def _run_build_harness_dry_run(repo: Path) -> subprocess.CompletedProcess[str]:
    env = os.environ.copy()
    env["PYTHONIOENCODING"] = "utf-8"
    return subprocess.run(
        [
            sys.executable,
            str(repo / "scripts" / "build_harness.py"),
            "--source",
            str(repo),
            "--dry-run",
        ],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        cwd=str(repo),
        env=env,
    )


class TestGetTreeHash:
    def test_real_commit_returns_tree(self):
        head = get_commit_hash(REPO_ROOT)
        assert head != "unknown"
        tree = get_tree_hash(head, REPO_ROOT)
        assert tree is not None
        assert len(tree) == 40

    def test_unknown_returns_none(self, tmp_path: Path):
        assert get_tree_hash("unknown", tmp_path) is None

    def test_empty_returns_none(self, tmp_path: Path):
        assert get_tree_hash("", tmp_path) is None

    def test_invalid_hash_returns_none(self, tmp_path: Path):
        result = get_tree_hash("deadbeefdeadbeefdeadbeefdeadbeefdeadbeef", tmp_path)
        assert result is None


class TestGetCommitHash:
    def test_git_repo_returns_hash(self):
        commit = get_commit_hash(REPO_ROOT)
        assert commit != "unknown"
        assert len(commit) == 40

    def test_non_git_dir_returns_unknown(self, tmp_path: Path):
        assert get_commit_hash(tmp_path) == "unknown"


class TestSourceInputIdempotency:
    @pytest.fixture
    def lock_backup(self, tmp_path: Path):
        lock_path = REPO_ROOT / ".harness" / ".adapter.lock"
        backup = tmp_path / "adapter.lock.backup"
        shutil.copy2(lock_path, backup)
        try:
            yield lock_path
        finally:
            shutil.copy2(backup, lock_path)

    def _write_lock(self, lock_path: Path, **overrides: object) -> None:
        data = json.loads(lock_path.read_text(encoding="utf-8"))
        data.update(
            {
                "adapter_version": ADAPTER_VERSION,
                "source_input_sha256": _source_input_hash(REPO_ROOT),
                "templateHash": _template_hash(REPO_ROOT),
            }
        )
        data.update(overrides)
        lock_path.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")

    def test_commit_diagnostic_change_does_not_force_reconvert(self, lock_backup: Path):
        self._write_lock(
            lock_backup,
            source_commit="deadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
        )

        result = _run_build_harness_dry_run(REPO_ROOT)

        assert result.returncode == 0
        assert "source inputs unchanged" in result.stdout
        assert "[SKIP]" in result.stdout

    def test_current_source_input_hash_skips(self, lock_backup: Path):
        self._write_lock(lock_backup)

        result = _run_build_harness_dry_run(REPO_ROOT)

        assert result.returncode == 0
        assert "source inputs unchanged" in result.stdout
        assert "[SKIP]" in result.stdout

    def test_source_input_hash_drift_triggers_reconvert(self, lock_backup: Path):
        self._write_lock(lock_backup, source_input_sha256="0" * 64)

        result = _run_build_harness_dry_run(REPO_ROOT)

        assert result.returncode == 0
        assert "Will re-convert" in result.stdout
        assert "[SKIP]" not in result.stdout

    def test_legacy_lock_without_source_input_hash_triggers_reconvert(self, lock_backup: Path):
        data = json.loads(lock_backup.read_text(encoding="utf-8"))
        data.pop("source_input_sha256", None)
        data["adapter_version"] = ADAPTER_VERSION
        data["templateHash"] = _template_hash(REPO_ROOT)
        lock_backup.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")

        result = _run_build_harness_dry_run(REPO_ROOT)

        assert result.returncode == 0
        assert "Will re-convert" in result.stdout


class TestHelpers:
    def test_template_hash_stable(self):
        tpl = REPO_ROOT / "scripts" / "templates" / "agent.md.template"
        h1 = template_hash(tpl)
        h2 = template_hash(tpl)
        assert h1 == h2
        assert len(h1) == 40

    def test_source_input_hash_stable(self):
        assert _source_input_hash(REPO_ROOT) == _source_input_hash(REPO_ROOT)
        assert len(_source_input_hash(REPO_ROOT)) == 64

    def test_read_adapter_lock_returns_dict(self):
        lock = REPO_ROOT / ".harness" / ".adapter.lock"
        assert read_adapter_lock(lock) is not None
        assert read_adapter_lock(REPO_ROOT / ".harness" / "non-existent.lock") is None

    def test_adapter_version_constant(self):
        lock = read_adapter_lock(REPO_ROOT / ".harness" / ".adapter.lock")
        if lock:
            assert lock.get("adapter_version") == ADAPTER_VERSION


class TestCleanupOldBak:
    def _make_baks(self, target: Path, count: int) -> list[Path]:
        baks = []
        for i in range(count):
            bak = target.with_name(f"{target.name}.bak.2026010{i}T000000")
            bak.write_text(f"old {i}", encoding="utf-8")
            os.utime(bak, (i, i))
            baks.append(bak)
        return baks

    def test_keeps_most_recent_n(self, tmp_path: Path):
        target = tmp_path / "agent.md"
        target.write_text("current", encoding="utf-8")
        self._make_baks(target, 5)
        assert cleanup_old_bak(target, keep=3) == 2
        assert len(sorted(target.parent.glob("agent.md.bak.*"))) == 3

    def test_no_bak_returns_zero(self, tmp_path: Path):
        target = tmp_path / "agent.md"
        target.write_text("current", encoding="utf-8")
        assert cleanup_old_bak(target, keep=3) == 0

    def test_keep_zero_deletes_all(self, tmp_path: Path):
        target = tmp_path / "agent.md"
        target.write_text("current", encoding="utf-8")
        self._make_baks(target, 3)
        assert cleanup_old_bak(target, keep=0) == 3
        assert list(target.parent.glob("agent.md.bak.*")) == []

    def test_fewer_than_keep_deletes_none(self, tmp_path: Path):
        target = tmp_path / "agent.md"
        target.write_text("current", encoding="utf-8")
        self._make_baks(target, 2)
        assert cleanup_old_bak(target, keep=5) == 0

    def test_negative_keep_no_op(self, tmp_path: Path):
        target = tmp_path / "agent.md"
        target.write_text("current", encoding="utf-8")
        self._make_baks(target, 3)
        assert cleanup_old_bak(target, keep=-1) == 0
        assert len(list(target.parent.glob("agent.md.bak.*"))) == 3
