from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import memory_gate, memory_store  # noqa: E402
from lib.gate_intercept import check_intercept  # noqa: E402


def _project(tmp_path: Path, *, phase: str = "coding", story: str = "STORY-001") -> Path:
    ae_sdd = tmp_path / ".ae-sdd"
    ae_sdd.mkdir()
    (ae_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
    (ae_sdd / "state.json").write_text(json.dumps({
        "version": "1",
        "projectKey": "test",
        "phase": phase,
        "currentStory": story,
        "currentTask": None,
        "history": [],
    }), encoding="utf-8")
    return ae_sdd


def test_memory_gate_blocks_missing_enter_write(tmp_path):
    ae_sdd = _project(tmp_path, phase="coding", story="STORY-001")
    result = memory_gate.check_state_transition(
        ade_sdd=ae_sdd,
        state_data={"phase": "coding", "currentStory": "STORY-001"},
        target_phase="test-running",
    )
    assert result["blocked"]
    assert result["memory_phase"] == "coding"
    assert "enter" in result["reason"]


def test_memory_gate_passes_after_enter_and_write(tmp_path):
    ae_sdd = _project(tmp_path, phase="coding", story="STORY-001")
    scope = memory_store.locate_scope(project=str(tmp_path), phase="coding", story="STORY-001")
    memory_store.enter(scope, actor="test")
    memory_store.write(scope, summary="Coding finished", evidence=["src/Foo.java:1"], actor="test")

    result = memory_gate.check_state_transition(
        ade_sdd=ae_sdd,
        state_data={"phase": "coding", "currentStory": "STORY-001"},
        target_phase="test-running",
    )
    assert result["pass"]
    assert not result["blocked"]


def test_gate_intercept_blocks_state_write_before_memory(tmp_path):
    _project(tmp_path, phase="coding", story="STORY-001")
    allowed, reason = check_intercept(
        "Bash",
        bash_command="ae-sdd state write --phase test-running",
        project_dir=tmp_path,
    )
    assert not allowed
    assert "Mandatory memory gate failed" in reason
    assert "memory phase: coding" in reason


def test_gate_intercept_reaches_entry_gates_after_memory_passes(tmp_path):
    _project(tmp_path, phase="coding", story="STORY-001")
    scope = memory_store.locate_scope(project=str(tmp_path), phase="coding", story="STORY-001")
    memory_store.enter(scope, actor="test")
    memory_store.write(scope, summary="Coding finished", evidence=["src/Foo.java:1"], actor="test")

    allowed, reason = check_intercept(
        "Bash",
        bash_command="ae-sdd state write --phase test-running",
        project_dir=tmp_path,
    )
    assert not allowed
    assert "Mandatory memory gate failed" not in reason
    assert "G-00" in reason


def test_cli_state_write_blocks_before_memory(tmp_path):
    _project(tmp_path, phase="coding", story="STORY-001")
    repo = Path(__file__).resolve().parent.parent.parent
    result = subprocess.run(
        [
            sys.executable,
            str(repo / "tools" / "bin" / "ae-sdd"),
            "state",
            "write",
            "--phase",
            "test-running",
            "--project-state",
        ],
        cwd=tmp_path,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 1
    assert "Mandatory memory gate failed" in (result.stdout + result.stderr)


def test_cli_state_write_allows_maintenance_override(tmp_path):
    _project(tmp_path, phase="coding", story="STORY-001")
    repo = Path(__file__).resolve().parent.parent.parent
    result = subprocess.run(
        [
            sys.executable,
            str(repo / "tools" / "bin" / "ae-sdd"),
            "state",
            "write",
            "--phase",
            "test-running",
            "--allow-empty-memory",
            "--project-state",
        ],
        cwd=tmp_path,
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0


def test_cli_memory_write_scope_project(tmp_path):
    _project(tmp_path, phase="coding", story="STORY-001")
    repo = Path(__file__).resolve().parent.parent.parent
    result = subprocess.run(
        [
            sys.executable,
            str(repo / "tools" / "bin" / "ae-sdd"),
            "memory",
            "write",
            "--scope",
            "project",
            "--phase",
            "coding",
            "--story",
            "STORY-001",
            "--kind",
            "constraint",
            "--summary",
            "Project-wide rule uses BigDecimal",
            "--evidence",
            "standards.md:2",
            "--json",
        ],
        cwd=tmp_path,
        capture_output=True,
        text=True,
        env={**os.environ, "PYTHONUTF8": "1"},
    )
    assert result.returncode == 0, result.stderr
    payload = json.loads(result.stdout)
    assert payload["record"]["memoryScope"] == "project"
    assert payload["record"]["layer"] == "L2"


# ─── 🆕 v3.8.2：is_scope_active 活跃态判断 ─────────────────────────────────


def test_is_scope_active_false_before_enter(tmp_path):
    """未 enter 时 scope 非活跃。"""
    _project(tmp_path, phase="coding", story="STORY-001")
    scope = memory_store.locate_scope(project=str(tmp_path), phase="coding", story="STORY-001")
    assert not memory_store.is_scope_active(scope)


def test_is_scope_active_true_after_enter(tmp_path):
    """enter 后未 exit 时 scope 活跃。"""
    _project(tmp_path, phase="coding", story="STORY-001")
    scope = memory_store.locate_scope(project=str(tmp_path), phase="coding", story="STORY-001")
    memory_store.enter(scope, actor="test")
    assert memory_store.is_scope_active(scope)


def test_is_scope_active_false_after_exit(tmp_path):
    """enter → write → exit 后 scope 非活跃（last_exit_at 写入）。"""
    _project(tmp_path, phase="coding", story="STORY-001")
    scope = memory_store.locate_scope(project=str(tmp_path), phase="coding", story="STORY-001")
    memory_store.enter(scope, actor="test")
    memory_store.write(scope, summary="done", evidence=["src/Foo.java:1"], actor="test")
    memory_store.exit_phase(scope, actor="test")
    assert not memory_store.is_scope_active(scope)


def test_is_scope_active_true_after_reenter(tmp_path):
    """exit 后重新 enter → scope 再次活跃（last_enter_at > last_exit_at）。"""
    _project(tmp_path, phase="coding", story="STORY-001")
    scope = memory_store.locate_scope(project=str(tmp_path), phase="coding", story="STORY-001")
    memory_store.enter(scope, actor="test")
    memory_store.write(scope, summary="done", evidence=["src/Foo.java:1"], actor="test")
    memory_store.exit_phase(scope, actor="test")
    assert not memory_store.is_scope_active(scope)
    memory_store.enter(scope, actor="test")
    assert memory_store.is_scope_active(scope)


def test_exit_phase_writes_last_exit_at(tmp_path):
    """exit_phase 成功后 .stage 文件含 last_exit_at 字段。"""
    _project(tmp_path, phase="coding", story="STORY-001")
    scope = memory_store.locate_scope(project=str(tmp_path), phase="coding", story="STORY-001")
    memory_store.enter(scope, actor="test")
    memory_store.write(scope, summary="done", evidence=["src/Foo.java:1"], actor="test")
    result = memory_store.exit_phase(scope, actor="test")
    assert result["pass"]
    assert result["stage"].get("last_exit_at") is not None


# ─── 🆕 v3.8.2：取端注入（prompt_inject） ──────────────────────────────────


def test_prompt_inject_no_memory_block_without_enter(tmp_path):
    """未 memory enter 时 prompt_inject 不注入 memory 块。"""
    _project(tmp_path, phase="coding", story="STORY-001")
    from lib.prompt_inject import _inject_memory_block
    block = _inject_memory_block(tmp_path / ".ae-sdd", "coding", "STORY-001")
    assert block is None


def test_prompt_inject_injects_memory_after_enter(tmp_path):
    """memory enter + write 后 prompt_inject 注入 compact memory 块。"""
    _project(tmp_path, phase="coding", story="STORY-001")
    scope = memory_store.locate_scope(project=str(tmp_path), phase="coding", story="STORY-001")
    memory_store.enter(scope, actor="test")
    memory_store.write(
        scope, summary="采用乐观锁防并发", kind="decision",
        evidence=["src/Order.java:42"], actor="test",
    )
    from lib.prompt_inject import _inject_memory_block
    block = _inject_memory_block(tmp_path / ".ae-sdd", "coding", "STORY-001")
    assert block is not None
    assert "MEMORY compact" in block
    assert "乐观锁" in block
    assert "src/Order.java:42" in block
    assert "[task decision]" in block


def test_prompt_inject_no_memory_block_after_exit(tmp_path):
    """memory exit 后 scope 非活跃，prompt_inject 不注入。"""
    _project(tmp_path, phase="coding", story="STORY-001")
    scope = memory_store.locate_scope(project=str(tmp_path), phase="coding", story="STORY-001")
    memory_store.enter(scope, actor="test")
    memory_store.write(scope, summary="done", evidence=["src/Foo.java:1"], actor="test")
    memory_store.exit_phase(scope, actor="test")
    from lib.prompt_inject import _inject_memory_block
    block = _inject_memory_block(tmp_path / ".ae-sdd", "coding", "STORY-001")
    assert block is None


def test_prompt_inject_skips_non_associated_phase(tmp_path):
    """非关联 phase（如 initialized）不注入 memory。"""
    _project(tmp_path, phase="initialized", story="STORY-001")
    from lib.prompt_inject import _inject_memory_block
    block = _inject_memory_block(tmp_path / ".ae-sdd", "initialized", "STORY-001")
    assert block is None


def test_prompt_inject_includes_l2_project_memory(tmp_path):
    """取端注入包含 L2 项目级记忆。"""
    _project(tmp_path, phase="coding", story="STORY-001")
    scope = memory_store.locate_scope(project=str(tmp_path), phase="coding", story="STORY-001")
    memory_store.enter(scope, actor="test")
    memory_store.write(
        scope, summary="Story 决策", layer="L1",
        kind="decision", evidence=["story.md:1"], actor="test",
    )
    memory_store.write(
        scope, summary="项目所有金额用 BigDecimal", layer="L2", kind="finding",
        evidence=["conventions.md:23"], actor="test",
    )
    from lib.prompt_inject import _inject_memory_block
    block = _inject_memory_block(tmp_path / ".ae-sdd", "coding", "STORY-001")
    assert block is not None
    assert "[project finding]" in block
    assert "BigDecimal" in block


def test_prompt_inject_prioritizes_task_memory_over_project_memory(tmp_path):
    """Task memory owns the injection budget; project memory only fills remaining slots."""
    _project(tmp_path, phase="coding", story="STORY-001")
    scope = memory_store.locate_scope(project=str(tmp_path), phase="coding", story="STORY-001")
    memory_store.enter(scope, actor="test")
    for i in range(9):
        memory_store.write(
            scope,
            summary=f"Task fact {i}",
            memory_scope="task",
            kind="finding",
            evidence=[f"task.md:{i + 1}"],
            actor="test",
        )
    memory_store.write(
        scope,
        summary="Project fact should not displace task facts",
        memory_scope="project",
        kind="constraint",
        evidence=["project.md:1"],
        actor="test",
    )

    from lib.prompt_inject import _inject_memory_block
    block = _inject_memory_block(tmp_path / ".ae-sdd", "coding", "STORY-001")
    assert block is not None
    assert "MEMORY compact task-first" in block
    assert "Task fact 8" in block
    assert "Project fact should not displace task facts" not in block


def test_prompt_inject_l0_events_do_not_squeeze_compact_memory(tmp_path):
    """L0 enter/scratch events must not hide L1/L2 compact memory from injection."""
    _project(tmp_path, phase="coding", story="STORY-001")
    scope = memory_store.locate_scope(project=str(tmp_path), phase="coding", story="STORY-001")
    memory_store.enter(scope, actor="test")
    memory_store.write(
        scope,
        summary="UserMapper now filters by tenant_id",
        memory_scope="task",
        kind="fix",
        evidence=["src/UserMapper.xml:31"],
        actor="test",
    )
    for i in range(12):
        memory_store.write(scope, summary=f"scratch {i}", layer="L0", actor="test")

    from lib.prompt_inject import _inject_memory_block
    block = _inject_memory_block(tmp_path / ".ae-sdd", "coding", "STORY-001")
    assert block is not None
    assert "UserMapper now filters by tenant_id" in block
    assert "scratch" not in block


# ─── 🆕 v3.8.2：存端兜底（gate_intercept 写源码未 enter 被拦） ──────────────


def test_store_gate_blocks_write_src_without_memory_enter(tmp_path):
    """关联 phase 写文件但未 memory enter → 被存端兜底拦截。

    用文档路径而非 src/，避免被关卡3（代码改动准入）先拦，隔离 memory 门禁的独立验证。
    """
    _project(tmp_path, phase="coding", story="STORY-001")
    allowed, reason = check_intercept(
        "Write",
        file_path=str(tmp_path / "docs" / "note.md"),
        project_dir=tmp_path,
    )
    assert not allowed
    assert "memory enter" in reason
    assert "ae-sdd memory enter --phase coding --story STORY-001" in reason


def test_store_gate_allows_write_after_memory_enter(tmp_path):
    """关联 phase 写文件且已 memory enter → 放行（不因 memory 被拦）。"""
    _project(tmp_path, phase="coding", story="STORY-001")
    scope = memory_store.locate_scope(project=str(tmp_path), phase="coding", story="STORY-001")
    memory_store.enter(scope, actor="test")
    allowed, reason = check_intercept(
        "Write",
        file_path=str(tmp_path / "docs" / "note.md"),
        project_dir=tmp_path,
    )
    # memory enter 后不再因 memory 门禁被拦（可能因其他门禁，但 reason 不含 memory enter）
    assert "memory enter" not in (reason or "")


def test_store_gate_skips_non_associated_phase(tmp_path):
    """非关联 phase（initialized）写操作不触发 memory 检查。"""
    _project(tmp_path, phase="initialized", story="STORY-001")
    allowed, reason = check_intercept(
        "Write",
        file_path=str(tmp_path / "docs/note.md"),
        project_dir=tmp_path,
    )
    # initialized 非关联 phase，不触发 memory 门禁（可能被其他门禁拦，但不因 memory）
    assert "memory enter" not in (reason or "")


def test_store_gate_blocks_all_associated_phases(tmp_path):
    """5 个关联 phase（ra/design/coding-plan/coding/review）写操作均触发 memory 检查。"""
    associated_state_phases = [
        ("ra-generated", "ra"),
        ("dr-generated", "design"),
        ("task-generated", "coding-plan"),
        ("coding", "coding"),
        ("code-reviewed", "review"),
    ]
    for state_phase, mem_phase in associated_state_phases:
        # 每轮重建 state.json（复用同一 .ae-sdd 目录）
        ae_sdd = tmp_path / ".ae-sdd"
        ae_sdd.mkdir(exist_ok=True)
        (ae_sdd / "config.yaml").write_text("projectKey: test\n", encoding="utf-8")
        (ae_sdd / "state.json").write_text(json.dumps({
            "version": "1", "projectKey": "test",
            "phase": state_phase, "currentStory": "STORY-001",
            "currentTask": None, "history": [],
        }), encoding="utf-8")
        allowed, reason = check_intercept(
            "Write",
            file_path=str(tmp_path / "docs" / "note.md"),
            project_dir=tmp_path,
        )
        assert not allowed, f"{state_phase} 未 memory enter 应被拦"
        assert f"--phase {mem_phase}" in reason, f"拦截原因应含 memory phase {mem_phase}: {reason}"
        # 清理 scope 状态，避免影响下一轮
        scope = memory_store.locate_scope(
            project=str(tmp_path), phase=mem_phase, story="STORY-001")
        stage_path = memory_store._stage_path(scope)
        if stage_path.exists():
            stage_path.unlink()
