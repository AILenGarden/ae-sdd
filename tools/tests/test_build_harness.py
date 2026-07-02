"""
test_build_harness.py — build_harness 模块单元测试（🆕 v3.5.6）

覆盖：
- get_tree_hash: 真实 commit → tree；unknown → None；非法 hash → None
- get_commit_hash: 真实 git 仓库返回非 unknown
- 幂等检查：lock 指向同 tree 的旧 commit（amend 场景）→ 应被 tree-hash 检查跳过
- 幂等检查：lock 指向不同 tree 的旧 commit（正常升级）→ 正常 drift 重转
- 幂等检查：lock == HEAD（无变更）→ 走"全一致"跳过
"""
from __future__ import annotations

import json
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

# build_harness.py 在 scripts/，不在 tools/lib/
sys.path.insert(0, str(Path(__file__).resolve().parent.parent.parent / "scripts"))
from build_harness import (  # noqa: E402
    ADAPTER_VERSION,
    cleanup_old_bak,
    get_commit_hash,
    get_tree_hash,
    read_adapter_lock,
    render_template,
    template_hash,
)


# ─── get_tree_hash 基础行为 ──────────────────────────────────────────────────

class TestGetTreeHash:
    def test_real_commit_returns_tree(self, tmp_path):
        """真实 git 仓库 + 真实 commit → 返回非 None tree hash"""
        # 用本仓库（已 git init）跑
        repo = Path(__file__).resolve().parent.parent.parent
        head = get_commit_hash(repo)
        assert head != "unknown"
        tree = get_tree_hash(head, repo)
        assert tree is not None
        assert len(tree) == 40  # SHA-1

    def test_unknown_returns_none(self, tmp_path):
        """commit='unknown' → 返回 None（避免对 'unknown^{tree}' 调 git）"""
        assert get_tree_hash("unknown", tmp_path) is None

    def test_empty_returns_none(self, tmp_path):
        """commit='' → 返回 None"""
        assert get_tree_hash("", tmp_path) is None

    def test_invalid_hash_returns_none(self, tmp_path):
        """不存在的 commit hash → 返回 None（git rev-parse 失败）"""
        result = get_tree_hash("deadbeefdeadbeefdeadbeefdeadbeefdeadbeef", tmp_path)
        assert result is None


# ─── get_commit_hash 基础行为 ──────────────────────────────────────────────────

class TestGetCommitHash:
    def test_git_repo_returns_hash(self, tmp_path):
        """真实 git 仓库 → 返回 HEAD hash（不是 'unknown'）"""
        repo = Path(__file__).resolve().parent.parent.parent
        commit = get_commit_hash(repo)
        assert commit != "unknown"
        assert len(commit) == 40

    def test_non_git_dir_returns_unknown(self, tmp_path):
        """非 git 目录 → 返回 'unknown'"""
        commit = get_commit_hash(tmp_path)
        assert commit == "unknown"


# ─── 幂等检查：tree-hash 一致性（🆕 v3.5.6 amend 循环修补） ──────────────────

class TestAmendDetection:
    """核心场景：lock 指向 amend 前的旧 commit（不同 hash 同 tree）→ 跳过重转"""

    @pytest.fixture
    def ae_sdd_repo(self):
        """使用真实 ae-sdd 母版仓库（已 git init 且至少 2 个 commit）"""
        return Path(__file__).resolve().parent.parent.parent

    def _make_fake_amend_commit(self, repo: Path) -> str:
        """构造 fake amend commit：同 tree 不同 hash（指向 HEAD~1 parent）"""
        head_tree = subprocess.run(
            ["git", "-C", str(repo), "rev-parse", "HEAD^{tree}"],
            capture_output=True, text=True, check=True,
        ).stdout.strip()
        head_parent = subprocess.run(
            ["git", "-C", str(repo), "rev-parse", "HEAD~1"],
            capture_output=True, text=True, check=True,
        ).stdout.strip()
        fake = subprocess.run(
            ["git", "-C", str(repo), "commit-tree", head_tree, "-p", head_parent,
             "-m", "fake amend test (transient)"],
            capture_output=True, text=True, check=True,
        ).stdout.strip()
        # 验证 fake 和 HEAD 同 tree
        fake_tree = subprocess.run(
            ["git", "-C", str(repo), "rev-parse", f"{fake}^{{tree}}"],
            capture_output=True, text=True, check=True,
        ).stdout.strip()
        assert fake_tree == head_tree, "fake commit 必须与 HEAD 同 tree"
        return fake

    def test_fake_amend_tree_match(self, ae_sdd_repo):
        """lock.commit = fake-amend hash（同 tree 不同 hash）→ tree 一致"""
        fake = self._make_fake_amend_commit(ae_sdd_repo)
        head_tree = get_tree_hash(get_commit_hash(ae_sdd_repo), ae_sdd_repo)
        fake_tree = get_tree_hash(fake, ae_sdd_repo)
        assert head_tree == fake_tree, "fake amend 必须满足 tree 一致性"

    def test_real_amend_scenario_skip(self, ae_sdd_repo):
        """集成测试：lock 指向 fake-amend hash → build_harness --dry-run 应跳过"""
        fake = self._make_fake_amend_commit(ae_sdd_repo)
        lock_path = ae_sdd_repo / ".harness" / ".adapter.lock"
        backup = Path("/tmp/ae-sdd.lock.backup.test_amend")
        shutil.copy2(lock_path, backup)

        try:
            data = json.loads(lock_path.read_text())
            data["commit"] = fake  # 改 lock 指向 fake-amend
            lock_path.write_text(json.dumps(data, ensure_ascii=False, indent=2))

            result = subprocess.run(
                [sys.executable, str(ae_sdd_repo / "scripts" / "build_harness.py"),
                 "--source", str(ae_sdd_repo), "--dry-run"],
                capture_output=True, text=True, cwd=str(ae_sdd_repo),
            )
            assert "tree-hash 一致" in result.stdout, (
                f"应被 tree-hash 检查跳过；实际输出:\n{result.stdout[:1500]}"
            )
            assert "[SKIP]" in result.stdout
        finally:
            shutil.copy2(backup, lock_path)


# ─── 幂等检查：lock == HEAD（无变更场景）─────────────────────────────────────

class TestFullIdempotency:
    """lock.commit == HEAD → 走原 4 维全一致检查（不影响 amend 逻辑）"""

    def test_lock_equals_head_skip(self):
        """lock 指向 HEAD → 走"全一致"分支跳过（不是 tree-hash 分支）"""
        repo = Path(__file__).resolve().parent.parent.parent
        lock_path = repo / ".harness" / ".adapter.lock"
        backup = Path("/tmp/ae-sdd.lock.backup.test_ide")
        shutil.copy2(lock_path, backup)

        try:
            head = get_commit_hash(repo)
            data = json.loads(lock_path.read_text())
            data["commit"] = head  # 与 HEAD 一致
            lock_path.write_text(json.dumps(data, ensure_ascii=False, indent=2))

            result = subprocess.run(
                [sys.executable, str(repo / "scripts" / "build_harness.py"),
                 "--source", str(repo), "--dry-run"],
                capture_output=True, text=True, cwd=str(repo),
            )
            assert "全部一致" in result.stdout, (
                f"应走 4 维全一致分支；实际:\n{result.stdout[:1500]}"
            )
            assert "[SKIP]" in result.stdout
        finally:
            shutil.copy2(backup, lock_path)


# ─── 幂等检查：正常升级（lock 指向不同 tree 的旧 commit）────────────────────

class TestNormalUpgrade:
    """lock.commit ≠ HEAD 且 tree 不同（真实升级）→ 正常 drift 重转（不跳过）"""

    def test_drift_triggers_reconvert(self):
        """lock 写一个不存在的 commit hash → drift 检测 → 应重转（DRY-RUN）"""
        repo = Path(__file__).resolve().parent.parent.parent
        lock_path = repo / ".harness" / ".adapter.lock"
        backup = Path("/tmp/ae-sdd.lock.backup.test_drift")
        shutil.copy2(lock_path, backup)

        try:
            data = json.loads(lock_path.read_text())
            data["commit"] = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef"  # 不存在
            lock_path.write_text(json.dumps(data, ensure_ascii=False, indent=2))

            result = subprocess.run(
                [sys.executable, str(repo / "scripts" / "build_harness.py"),
                 "--source", str(repo), "--dry-run"],
                capture_output=True, text=True, cwd=str(repo),
            )
            assert "Will re-convert" in result.stdout, (
                f"应触发重转；实际:\n{result.stdout[:1500]}"
            )
            assert "[SKIP]" not in result.stdout
        finally:
            shutil.copy2(backup, lock_path)


# ─── read_adapter_lock + template_hash 健全性 ─────────────────────────────────

class TestHelpers:
    def test_template_hash_stable(self):
        """同一模板文件 → 同一 hash（SHA-1）"""
        repo = Path(__file__).resolve().parent.parent.parent
        tpl = repo / "scripts" / "templates" / "agent.md.template"
        h1 = template_hash(tpl)
        h2 = template_hash(tpl)
        assert h1 == h2
        assert len(h1) == 40

    def test_read_adapter_lock_returns_dict(self):
        """存在 lock → 返回 dict；不存在 → 返回 None"""
        repo = Path(__file__).resolve().parent.parent.parent
        lock = repo / ".harness" / ".adapter.lock"
        assert read_adapter_lock(lock) is not None
        assert read_adapter_lock(repo / ".harness" / "non-existent.lock") is None

    def test_adapter_version_constant(self):
        """ADAPTER_VERSION 与 .adapter.lock 中一致（防止漂移）"""
        repo = Path(__file__).resolve().parent.parent.parent
        lock = read_adapter_lock(repo / ".harness" / ".adapter.lock")
        if lock:
            assert lock.get("adapter_version") == ADAPTER_VERSION


# ─── cleanup_old_bak 备份轮转（🆕 治 K2：agent.md.bak.* 无限累积）────────────

class TestCleanupOldBak:
    """备份轮转：保留最近 keep 个 .bak.<ts>，删其余。"""

    def _make_baks(self, target: Path, count: int) -> list:
        """造 count 个 .bak.<ts> 文件（用不同时间戳，从旧到新）。"""
        baks = []
        for i in range(count):
            bak = target.with_name(f"{target.name}.bak.2026010{i}T000000")
            bak.write_text(f"old {i}", encoding="utf-8")
            # 用 os.utime 设 mtime 让排序确定（i 越大越新）
            import os
            os.utime(bak, (i, i))
            baks.append(bak)
        return baks

    def test_keeps_most_recent_n(self, tmp_path):
        """5 个 bak + keep=3 → 删 2 个最旧，留 3 个最新"""
        target = tmp_path / "agent.md"
        target.write_text("current", encoding="utf-8")
        self._make_baks(target, 5)
        removed = cleanup_old_bak(target, keep=3)
        assert removed == 2
        remaining = sorted(target.parent.glob("agent.md.bak.*"))
        assert len(remaining) == 3

    def test_no_bak_returns_zero(self, tmp_path):
        """无 bak 文件 → 删 0，不报错"""
        target = tmp_path / "agent.md"
        target.write_text("current", encoding="utf-8")
        assert cleanup_old_bak(target, keep=3) == 0

    def test_keep_zero_deletes_all(self, tmp_path):
        """keep=0 → 删全部 bak"""
        target = tmp_path / "agent.md"
        target.write_text("current", encoding="utf-8")
        self._make_baks(target, 3)
        assert cleanup_old_bak(target, keep=0) == 3
        assert list(target.parent.glob("agent.md.bak.*")) == []

    def test_fewer_than_keep_deletes_none(self, tmp_path):
        """bak 数 < keep → 不删"""
        target = tmp_path / "agent.md"
        target.write_text("current", encoding="utf-8")
        self._make_baks(target, 2)
        assert cleanup_old_bak(target, keep=5) == 0

    def test_negative_keep_no_op(self, tmp_path):
        """keep<0 → 安全无操作（防误删）"""
        target = tmp_path / "agent.md"
        target.write_text("current", encoding="utf-8")
        self._make_baks(target, 3)
        assert cleanup_old_bak(target, keep=-1) == 0
        assert len(list(target.parent.glob("agent.md.bak.*"))) == 3
