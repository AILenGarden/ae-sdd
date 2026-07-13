"""
test_memory_gate.py - memory gate + integration tests (🆕 v3.10.3).

v3.10.3: memory_gate 改为 passthrough（check_state_transition 永远 pass），
memory_store 重写为业务实体树。本文件验证：
  1. memory_gate passthrough 行为
  2. memory_store 新 API 集成
  3. prompt_inject 从 memory 读 compact 注入
  4. gate_intercept memory 命令放行通道
"""
from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
sys.path.insert(0, str(REPO_ROOT / "tools"))

import pytest  # type: ignore
from lib import memory_gate, memory_store  # noqa: E402


# ─── memory_gate passthrough (v3.10.3) ──────────────────────────────────────

class TestMemoryGatePassthrough:
    """🆕 v3.10.3: memory_gate.check_state_transition always passes."""

    def test_check_state_transition_always_passes(self, tmp_path):
        result = memory_gate.check_state_transition(
            ade_sdd=tmp_path / ".ae-sdd",
            state_data={"phase": "coding", "scale": "微"},
            target_phase="test-running",
        )
        assert result["pass"] is True
        assert result["blocked"] is False
        assert result["skipped"] is True

    def test_check_state_transition_passes_without_ae_sdd(self):
        result = memory_gate.check_state_transition(
            ade_sdd=None,
            state_data={"phase": "initialized"},
            target_phase="coding",
        )
        assert result["pass"] is True

    def test_format_transition_block_returns_empty(self):
        check = {"current_phase": "coding", "target_phase": "review", "memory_phase": "coding"}
        assert memory_gate.format_transition_block(check) == ""

    def test_memory_phase_for_state_phase_delegates_to_store(self):
        """memory_gate.memory_phase_for_state_phase 委托 memory_store。"""
        assert memory_gate.memory_phase_for_state_phase("coding") == "coding"
        assert memory_gate.memory_phase_for_state_phase("ra-generated") == "ra"
        assert memory_gate.memory_phase_for_state_phase("initialized") is None


# ─── memory_store new API integration ───────────────────────────────────────

class TestMemoryStoreIntegration:

    def test_create_and_read_memory(self, tmp_path):
        scope = memory_store.locate_scope(
            project=str(tmp_path), entity_type="story", entity_id="STORY-001-BE",
        )
        memory_store.create_memory(
            scope,
            source_contexts={"constraints": "Use BigDecimal."},
            current_series="story-generate",
            next_step="generate",
            constraints=["BigDecimal"],
        )
        mem = memory_store.read_memory(scope)
        assert "story-generate" in mem["boot"]
        assert "BigDecimal" in mem["context"]

    def test_is_scope_active_equals_exists(self, tmp_path):
        """🆕 v3.10.3: is_scope_active 新语义 = exists_memory。"""
        scope = memory_store.locate_scope(
            project=str(tmp_path), entity_type="coding", entity_id="STORY-001-BE",
        )
        assert memory_store.is_scope_active(scope) is False
        memory_store.create_memory(scope, source_contexts={})
        assert memory_store.is_scope_active(scope) is True
        memory_store.clean_memory(scope)
        assert memory_store.is_scope_active(scope) is False

    def test_clean_all_preserves_common(self, tmp_path):
        scope = memory_store.locate_scope(
            project=str(tmp_path), entity_type="story", entity_id="STORY-001",
        )
        memory_store.create_memory(
            scope, source_contexts={"constraints": "禁止大事务。BigDecimal mandatory."}
        )
        result = memory_store.clean_all_memory(scope)
        assert "common" in result["preserved"]
        common = memory_store.read_common(scope)
        assert common  # common survives clean-all


# ─── prompt_inject memory block (v3.10.3) ───────────────────────────────────

class TestPromptInjectMemory:

    def test_no_memory_block_when_memory_absent(self, tmp_path):
        """memory 不存在时 _inject_memory_block 返回 None。"""
        from lib import prompt_inject
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        result = prompt_inject._inject_memory_block(ade_sdd, "coding", "STORY-001-BE")
        assert result is None

    def test_injects_memory_when_exists(self, tmp_path):
        """memory 存在时注入 compact 内容。"""
        from lib import prompt_inject
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        scope = memory_store.locate_scope(
            project=str(tmp_path), entity_type="coding", entity_id="STORY-001-BE",
        )
        memory_store.create_memory(
            scope,
            source_contexts={},
            current_series="coding",
            next_step="write code",
            constraints=["BigDecimal"],
        )
        result = prompt_inject._inject_memory_block(ade_sdd, "coding", "STORY-001-BE")
        assert result is not None
        assert "coding/STORY-001-BE" in result
        assert "BigDecimal" in result
        assert "## Boot" in result
        assert "## Context" in result

    def test_skips_non_associated_phase(self, tmp_path):
        """非关联 phase（如 initialized）不注入。"""
        from lib import prompt_inject
        ade_sdd = tmp_path / ".ae-sdd"
        ade_sdd.mkdir()
        result = prompt_inject._inject_memory_block(ade_sdd, "initialized", "STORY-001-BE")
        assert result is None


# ─── gate_intercept memory command passthrough (v3.10.3) ────────────────────

class TestGateInterceptMemoryCmd:

    def test_memory_cmd_regex_matches(self):
        """_MEMORY_CMD_RE 仍匹配 ae-sdd memory 命令（放行通道保留）。"""
        from lib import gate_intercept
        assert gate_intercept._is_ae_sdd_memory_cmd("ae-sdd memory create --entity-type story")
        assert gate_intercept._is_ae_sdd_memory_cmd("ae-sdd memory read --entity-type story")
        assert gate_intercept._is_ae_sdd_memory_cmd("ae-sdd memory clean --entity-type story")
        assert gate_intercept._is_ae_sdd_memory_cmd("python tools/bin/ae-sdd memory search --query test")
        assert not gate_intercept._is_ae_sdd_memory_cmd("ae-sdd state write --phase coding")
