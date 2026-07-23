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
    assert_independent_identity,
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


class TestIdentitySanityCheck:
    """assert_independent_identity 正则白名单覆盖测试（v3.9.9 补，Out of scope #2）。

    每条 pattern 都需要：
    1. 命中用例  → 期待 SystemExit(2)
    2. 误报用例  → 合法提及宿主名，期待 pass（不抛）
    """

    def _assert_raises(self, text: str) -> None:
        with pytest.raises(SystemExit) as exc_info:
            assert_independent_identity(text, context="test")
        assert exc_info.value.code == 2

    def _assert_pass(self, text: str) -> None:
        # 不应抛异常
        assert_independent_identity(text, context="test")

    # ── 干净内容 ────────────────────────────────────────────────────────────

    def test_clean_content_passes(self):
        self._assert_pass(
            "You are the ae-sdd skill — a client-agnostic Skill running on any host."
        )

    # ── Pattern 1: ae-sdd ... Harness 归属句 ─────────────────────────────────

    def test_pattern1_ae_sdd_is_harness(self):
        self._assert_raises("ae-sdd is the Harness orchestrator.")

    def test_pattern1_ae_sdd_harness_variant(self):
        self._assert_raises("ae-sdd acts as Harness sub-agent.")

    # ── Pattern 2: Harness format/mode/role ────────────────────────────

    def test_pattern2_harness_format(self):
        self._assert_raises("This output is in Harness format.")

    def test_pattern2_harness_role(self):
        self._assert_raises("Harness role is to orchestrate tasks.")

    # ── Pattern 3: you are the ... orchestrator ──────────────────────────────

    def test_pattern3_you_are_the_orchestrator(self):
        self._assert_raises("You are the auto-engineering orchestrator for this project.")

    def test_pattern3_you_are_orchestrator_mixed_case(self):
        self._assert_raises("you are the Orchestrator that drives ae-sdd.")

    # ── Pattern 4: ae-sdd ... orchestrator ──────────────────────────────────

    def test_pattern4_ae_sdd_orchestrator(self):
        self._assert_raises("ae-sdd is an orchestrator embedded in Harness.")

    def test_pattern4_aesdd_orchestrator_no_dash(self):
        self._assert_raises("aesdd orchestrator runs inside the host.")

    # ── Pattern 5: auto-engineering orchestrator ─────────────────────────────

    def test_pattern5_auto_engineering_orchestrator(self):
        self._assert_raises("Welcome to the auto-engineering orchestrator.")

    def test_pattern5_autoengineering_orchestrator_space(self):
        self._assert_raises("auto engineering orchestrator is ready.")

    # ── 合法提及（误报防护）─────────────────────────────────────────────────

    def test_no_false_positive_harness_as_host_description(self):
        """合法描述：ae-sdd 运行于 Harness 之上，不是归属声明。"""
        self._assert_pass(
            "ae-sdd runs on top of the Harness via `harness mount`."
        )

    def test_no_false_positive_harness_mount_command(self):
        """命令行示例中出现 harness 不应被误报。"""
        self._assert_pass("Run: harness mount ~/.ae-sdd/harness")

    def test_no_false_positive_orchestration_as_noun(self):
        """orchestration（名词）≠ orchestrator（角色声明），不应触发。"""
        self._assert_pass("ae-sdd supports multi-agent orchestration workflows.")
