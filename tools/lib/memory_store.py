"""
Entity-tree memory store for ae-sdd.

🆕 v3.10.3: memory 从"5层原文索引"重构为"业务实体树+编译文档容器"。

核心变化：
  - 废弃 5 层（L0-L4）+ JSONL 原文索引 + enter/exit 生命周期门禁。
  - 新增业务实体树：prd/dr/story/testcase/coding/common 平级分层。
  - 存储格式：compact.md 文件（编译后的高密度文档）+ manifest.json 校验。
  - 生命周期：子流程启动=创建(编译)，结束=删除；从0重建=clean_all；回归=先读无则建。
  - common 层：只存项目级可复用约束，跨子流程保留，必须轻。

目录结构：
  .ae-sdd/memory/
  ├── common/                  # 项目级可复用约束(必须轻)，跨子流程保留
  │   └── context.compact.md
  ├── prd/{PRD-ID}/            # RA/PRD子流程的工作上下文
  │   ├── boot.compact.md
  │   ├── context.compact.md
  │   ├── pending.compact.md
  │   └── manifest.json
  ├── dr/{DR-ID}/              # DR子流程的工作上下文
  ├── story/{Story-ID}/        # Story子流程的工作上下文
  ├── testcase/{Story-ID}/     # TestCase子流程的工作上下文
  └── coding/{Story-ID}/       # Coding子流程的工作上下文

兼容层：
  - locate_scope() 保留但语义简化为定位 memory_root（过渡期供旧调用方使用）。
  - memory_phase_for_state_phase() 从 memory_gate 迁入本模块（过渡期供 prompt_inject/gate_intercept 使用）。
"""
from __future__ import annotations

import json
import shutil
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

from lib import memory_compiler, paths


# --- entity types ---

VALID_ENTITY_TYPES = {"prd", "dr", "story", "testcase", "coding", "common"}

# state phase -> memory entity type 映射（过渡期，供 prompt_inject/gate_intercept 判定当前阶段对应哪个实体）。
# 🆕 v3.10.3: 从 memory_gate.py 迁入（memory_gate 废弃）。
STATE_PHASE_TO_ENTITY_TYPE: dict[str, str] = {
    "ra-generated": "prd",
    "dr-generated": "dr",
    "story-generated": "story",
    "story-reviewed": "story",
    "testcase-generated": "testcase",
    "testcase-reviewed": "testcase",
    "coding-process": "coding",
    "coding": "coding",
    "test-running": "coding",
    "code-reviewed": "coding",
}


def entity_type_for_state_phase(phase: str) -> Optional[str]:
    """state phase -> memory entity type 映射（None 表示该阶段无关联实体）。"""
    return STATE_PHASE_TO_ENTITY_TYPE.get(phase)


# 旧 memory_gate.STATE_PHASE_TO_MEMORY_PHASE 的兼容别名（过渡期）。
# 5 个 memory phase: ra/design/coding-plan/coding/review。
# 🆕 v3.10.3: 保留供 prompt_inject/gate_intercept 过渡期使用，后续批 3 重写后可移除。
_STATE_PHASE_TO_MEMORY_PHASE: dict[str, str] = {
    "ra-generated": "ra",
    "dr-generated": "design",
    "story-generated": "design",
    "story-reviewed": "design",
    "task-generated": "coding-plan",
    "task-reviewed": "coding-plan",
    "coding-process": "coding-plan",
    "coding": "coding",
    "test-running": "coding",
    "code-reviewed": "review",
}

_VALID_MEMORY_PHASES = {"ra", "design", "coding-plan", "coding", "review"}


def memory_phase_for_state_phase(phase: str) -> Optional[str]:
    """state phase -> memory phase 映射（过渡期兼容，供 prompt_inject/gate_intercept 使用）。

    🆕 v3.10.3: 从 memory_gate.py 迁入。批 3 重写 prompt_inject/gate_intercept 后可移除。
    """
    return _STATE_PHASE_TO_MEMORY_PHASE.get(phase)


# --- scope ---

@dataclass
class MemoryScope:
    """业务实体 scope（🆕 v3.10.3 替代旧 phase-based MemoryScope）。

    entity_type: prd/dr/story/testcase/coding/common
    entity_id: 业务实体 ID（如 STORY-001-BE、DR-001、PRD-CS-001）
    """
    project_root: Path
    memory_root: Path
    entity_type: str
    entity_id: str

    @property
    def scope_key(self) -> str:
        """兼容 property：供过渡期旧调用方（memory_gate 等）读取。"""
        return f"{self.entity_type}__{self.entity_id}"

    @property
    def entity_dir(self) -> Path:
        return self.memory_root / self.entity_type / _safe_part(self.entity_id)


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _safe_part(value: str) -> str:
    return "".join(ch if ch.isalnum() or ch in ("-", "_", ".") else "_" for ch in value)


def locate_scope(
    *,
    project: Optional[str] = None,
    entity_type: Optional[str] = None,
    entity_id: Optional[str] = None,
    # 过渡期兼容旧参数（phase/story/task），内部转换为 entity_type/entity_id
    phase: Optional[str] = None,
    story: Optional[str] = None,
    task: Optional[str] = None,
) -> MemoryScope:
    """定位 memory scope。

    🆕 v3.10.3: 新参数 entity_type/entity_id。旧参数 phase/story/task 过渡期兼容，
    内部转换为 entity_type/entity_id（phase->entity_type 映射 + story->entity_id）。
    """
    project_dir = Path(project).resolve() if project else Path.cwd()
    ade_sdd = paths.locate_project_ae_sdd(project_dir)
    if ade_sdd is None:
        ade_sdd = project_dir / ".ae-sdd"
    project_root = ade_sdd.parent
    memory_root = ade_sdd / "memory"

    # 过渡期：旧参数 phase/story/task -> entity_type/entity_id
    # 优先级：显式 entity_type/entity_id > 旧 phase/story/task > 默认 common/default
    if not entity_type and phase:
        entity_type = entity_type_for_state_phase(phase) or "common"
    if not entity_id and story:
        entity_id = story
    if not entity_type and task:
        # 旧 task 维度映射到 coding 实体
        entity_type = "coding"
    if not entity_id and task:
        entity_id = task

    # 默认值
    entity_type = entity_type or "common"
    entity_id = entity_id or "default"

    if entity_type not in VALID_ENTITY_TYPES:
        raise ValueError(
            f"unknown entity type: {entity_type} (allowed: {sorted(VALID_ENTITY_TYPES)})"
        )

    return MemoryScope(
        project_root=project_root,
        memory_root=memory_root,
        entity_type=entity_type,
        entity_id=entity_id,
    )


# --- path helpers ---

def _compact_path(scope: MemoryScope, slice_name: str) -> Path:
    """memory/{entity_type}/{entity_id}/{slice_name}.compact.md"""
    return scope.entity_dir / f"{slice_name}.compact.md"


def _manifest_path(scope: MemoryScope) -> Path:
    return scope.entity_dir / "manifest.json"


# --- read/write helpers ---

def _write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="\n")


def _read_text(path: Path) -> str:
    if not path.is_file():
        return ""
    return path.read_text(encoding="utf-8")


def _read_json(path: Path) -> dict:
    if not path.is_file():
        return {}
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return {}


def _write_json(path: Path, data: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(data, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


# --- core API: create/read/update/clean ---

def create_memory(
    scope: MemoryScope,
    *,
    source_contexts: dict[str, str],
    series_chain: list[str] | None = None,
    current_series: str = "",
    next_step: str = "",
    deliverables: list[dict[str, str]] | None = None,
    dr_anchors: list[dict[str, str]] | None = None,
    story_acs: list[dict[str, str]] | None = None,
    constraints: list[str] | None = None,
    api_contracts: list[dict[str, str]] | None = None,
    data_models: list[dict[str, str]] | None = None,
    asset_refs: list[str] | None = None,
    pending_items: list[dict[str, str]] | None = None,
    failure_history: list[dict[str, str]] | None = None,
    correction_counts: dict[str, int] | None = None,
    review_loop_status: str = "",
) -> dict:
    """读源上下文 -> 编译 compact -> 写 4 文件 -> 返回路径信息。

    这是子流程Agent首次进入时调用的主入口。编译由 memory_compiler 完成。
    """
    compiled = memory_compiler.compile_source_to_memory(
        entity_type=scope.entity_type,
        entity_id=scope.entity_id,
        source_contexts=source_contexts,
        series_chain=series_chain,
        current_series=current_series,
        next_step=next_step,
        deliverables=deliverables,
        dr_anchors=dr_anchors,
        story_acs=story_acs,
        constraints=constraints,
        api_contracts=api_contracts,
        data_models=data_models,
        asset_refs=asset_refs,
        pending_items=pending_items,
        failure_history=failure_history,
        correction_counts=correction_counts,
        review_loop_status=review_loop_status,
    )

    for filename, content in compiled.items():
        _write_text(scope.entity_dir / filename, content)

    # 提取 common（若当前实体不是 common 本身）
    if scope.entity_type != "common":
        _maybe_update_common(scope, source_contexts)

    return {
        "created": True,
        "entity_type": scope.entity_type,
        "entity_id": scope.entity_id,
        "path": str(scope.entity_dir),
        "slices": list(compiled.keys()),
    }


def _maybe_update_common(scope: MemoryScope, source_contexts: dict[str, str]) -> None:
    """从源上下文提取可复用约束，更新 common（若 common 不存在则创建）。"""
    common_scope = MemoryScope(
        project_root=scope.project_root,
        memory_root=scope.memory_root,
        entity_type="common",
        entity_id="default",
    )
    common_context = memory_compiler.extract_common(source_contexts)
    common_context_path = _compact_path(common_scope, "context")

    if not common_context_path.is_file():
        # common 不存在，创建
        _write_text(common_context_path, common_context)
    # 若已存在，不覆盖（common 由首次编译创建，后续保留跨子流程复用）


def read_memory(scope: MemoryScope) -> dict:
    """读 compact 文档（boot + context + pending + manifest）。

    返回 dict 含各 slice 的文本内容。若 memory 不存在返回空 dict。
    """
    if not scope.entity_dir.is_dir():
        return {}

    return {
        "boot": _read_text(_compact_path(scope, "boot")),
        "context": _read_text(_compact_path(scope, "context")),
        "pending": _read_text(_compact_path(scope, "pending")),
        "manifest": _read_json(_manifest_path(scope)),
        "path": str(scope.entity_dir),
    }


def update_memory(
    scope: MemoryScope,
    *,
    slice_name: str,
    content: str,
) -> dict:
    """增量更新某个 slice（如更新 pending.compact.md 的待决项）。

    slice_name: boot/context/pending（不含 .compact.md 后缀）。
    """
    if slice_name not in ("boot", "context", "pending"):
        raise ValueError(f"slice_name must be boot/context/pending, got: {slice_name}")
    if not scope.entity_dir.is_dir():
        raise FileNotFoundError(
            f"memory not found for {scope.entity_type}/{scope.entity_id}; "
            "call create_memory first"
        )
    path = _compact_path(scope, slice_name)
    _write_text(path, content)

    # 更新 manifest 的 slice hash
    manifest = _read_json(_manifest_path(scope))
    if manifest and "slices" in manifest:
        slice_key = slice_name
        if slice_key in manifest["slices"]:
            import hashlib
            manifest["slices"][slice_key]["sha256"] = hashlib.sha256(
                content.encode("utf-8")
            ).hexdigest()
            # 重算 fingerprint
            fingerprint_payload = {
                "entity_type": scope.entity_type,
                "entity_id": scope.entity_id,
                "source_hashes": manifest.get("source_hashes", {}),
                "boot_sha256": manifest["slices"].get("boot", {}).get("sha256", ""),
                "context_sha256": manifest["slices"].get("context", {}).get("sha256", ""),
                "pending_sha256": manifest["slices"].get("pending", {}).get("sha256", ""),
            }
            import json as _json
            fingerprint_input = _json.dumps(
                fingerprint_payload, ensure_ascii=False, sort_keys=True, separators=(",", ":")
            )
            manifest["fingerprint"] = hashlib.sha256(
                fingerprint_input.encode("utf-8")
            ).hexdigest()
            _write_json(_manifest_path(scope), manifest)

    return {
        "updated": True,
        "slice": slice_name,
        "path": str(path),
    }


def clean_memory(scope: MemoryScope) -> dict:
    """删单个实体的 memory（子流程结束时调用）。

    删除 entity_dir 下所有文件。common 不在此删除（跨子流程保留）。
    """
    if scope.entity_type == "common":
        return {
            "cleaned": False,
            "reason": "common memory is preserved across subprocesses; use clean_common() to force remove",
        }
    if not scope.entity_dir.is_dir():
        return {"cleaned": False, "reason": "memory dir not found", "path": str(scope.entity_dir)}
    shutil.rmtree(scope.entity_dir)
    return {"cleaned": True, "path": str(scope.entity_dir)}


def clean_all_memory(scope: MemoryScope) -> dict:
    """删所有实体的 memory（从0重新开始时调用）。

    删除 memory_root 下所有实体目录（prd/dr/story/testcase/coding），
    但保留 common（跨子流程复用的项目级约束）。
    """
    cleaned: list[str] = []
    if not scope.memory_root.is_dir():
        return {"cleaned": False, "reason": "memory_root not found"}

    for entity_type in ("prd", "dr", "story", "testcase", "coding"):
        entity_type_dir = scope.memory_root / entity_type
        if entity_type_dir.is_dir():
            shutil.rmtree(entity_type_dir)
            cleaned.append(entity_type)

    return {
        "cleaned": True,
        "removed_types": cleaned,
        "preserved": ["common"],
        "path": str(scope.memory_root),
    }


def clean_common(scope: MemoryScope) -> dict:
    """强制删除 common memory（仅在显式重置项目级约束时使用）。"""
    common_dir = scope.memory_root / "common"
    if not common_dir.is_dir():
        return {"cleaned": False, "reason": "common dir not found"}
    shutil.rmtree(common_dir)
    return {"cleaned": True, "path": str(common_dir)}


def exists_memory(scope: MemoryScope) -> bool:
    """检查 memory 是否存在（回归流程时先检查，有则读无则建）。

    普通实体：检查 entity_dir + manifest.json。
    common：只有 context.compact.md（无 manifest），检查 entity_dir + context.compact.md。
    """
    if not scope.entity_dir.is_dir():
        return False
    if scope.entity_type == "common":
        return _compact_path(scope, "context").is_file()
    return _manifest_path(scope).is_file()


# --- common API ---

def read_common(scope: MemoryScope) -> dict:
    """读 common memory（跨子流程复用的项目级约束）。"""
    common_scope = MemoryScope(
        project_root=scope.project_root,
        memory_root=scope.memory_root,
        entity_type="common",
        entity_id="default",
    )
    return read_memory(common_scope)


def update_common(scope: MemoryScope, *, content: str) -> dict:
    """更新 common memory 的 context.compact.md。"""
    common_scope = MemoryScope(
        project_root=scope.project_root,
        memory_root=scope.memory_root,
        entity_type="common",
        entity_id="default",
    )
    common_scope.entity_dir.mkdir(parents=True, exist_ok=True)
    _write_text(_compact_path(common_scope, "context"), content)
    return {"updated": True, "path": str(_compact_path(common_scope, "context"))}


# --- compact snapshot (for Task 1: compact reload) ---

def pre_compact_snapshot(
    scope: MemoryScope,
    *,
    current_series: str,
    next_step: str,
    pending_items: list[dict[str, str]],
    failure_history: list[dict[str, str]] | None = None,
    correction_counts: dict[str, int] | None = None,
    review_loop_status: str = "",
) -> dict:
    """compact 前调用：把当前系列进度/待决项写入 memory。

    更新 boot.compact.md（current_series/next_step）+ pending.compact.md（进度/待决项）。
    """
    if not scope.entity_dir.is_dir():
        raise FileNotFoundError(
            f"memory not found for {scope.entity_type}/{scope.entity_id}; "
            "call create_memory first"
        )

    # 更新 boot（current_series/next_step）
    manifest = _read_json(_manifest_path(scope))
    boot_text = _read_text(_compact_path(scope, "boot"))
    # 简单更新：重写 boot 的 current_series/next_step 行
    import re
    boot_text = re.sub(
        r"- current_series:.*",
        f"- current_series: {current_series}",
        boot_text,
    )
    boot_text = re.sub(
        r"- next_step:.*",
        f"- next_step: {next_step}",
        boot_text,
    )
    _write_text(_compact_path(scope, "boot"), boot_text)

    # 重写 pending
    pending_text = memory_compiler.render_pending_compact(
        pending_items=pending_items,
        failure_history=failure_history or [],
        correction_counts=correction_counts or {},
        review_loop_status=review_loop_status,
    )
    _write_text(_compact_path(scope, "pending"), pending_text)

    return {
        "snapshotted": True,
        "current_series": current_series,
        "next_step": next_step,
        "pending_count": len(pending_items),
    }


def post_compact_reload(scope: MemoryScope) -> dict:
    """compact 后调用：从 memory 重载完整上下文。

    返回 read_memory() 的结果（boot + context + pending + manifest）。
    """
    return read_memory(scope)


# --- search/summarize (adapted for new structure) ---

def search_memory(
    scope: MemoryScope,
    *,
    query: str,
    limit: int = 20,
) -> list[dict]:
    """跨 memory 实体搜索（子串匹配 compact.md 内容）。"""
    q = (query or "").lower()
    if not q:
        return []
    results: list[dict] = []
    if not scope.memory_root.is_dir():
        return []
    for md_path in scope.memory_root.rglob("*.compact.md"):
        content = md_path.read_text(encoding="utf-8", errors="replace")
        if q in content.lower():
            results.append({
                "path": str(md_path),
                "entity": md_path.parent.name,
                "slice": md_path.stem,
                "snippet": content[:200],
            })
        if len(results) >= limit:
            break
    return results


def summarize_memory(scope: MemoryScope) -> dict:
    """统计 memory 目录下各实体的文件数。"""
    counts: dict[str, int] = {}
    total = 0
    if scope.memory_root.is_dir():
        for entity_type_dir in scope.memory_root.iterdir():
            if not entity_type_dir.is_dir():
                continue
            entity_count = 0
            for md_path in entity_type_dir.rglob("*.compact.md"):
                entity_count += 1
                total += 1
            counts[entity_type_dir.name] = entity_count
    return {
        "total_slices": total,
        "by_entity_type": counts,
        "root": str(scope.memory_root),
    }


# --- 过渡期兼容 API（供旧调用方逐步迁移） ---
# 以下函数保留旧签名但行为适配新结构，供 prompt_inject/gate_intercept/CLI 过渡期使用。
# 批 3 重写 prompt_inject/gate_intercept 后可移除。

def is_scope_active(scope: MemoryScope) -> bool:
    """过渡期兼容：检查 memory 是否存在（存在=活跃）。

    🆕 v3.10.3: 新语义下"活跃"="memory 存在"（子流程启动创建，结束删除）。
    旧语义是 enter/exit 状态机，新语义简化为存在性检查。
    """
    return exists_memory(scope)


def read(
    scope: MemoryScope,
    *,
    include_project: bool = True,
    limit: int = 20,
    memory_scope: Optional[str] = None,
) -> list[dict]:
    """过渡期兼容：读 memory 内容作为 list[dict]（旧 read() 返回格式）。

    新代码应直接用 read_memory() 获取 compact 文档。
    """
    mem = read_memory(scope)
    if not mem:
        # 若 include_project，也读 common
        if include_project:
            common = read_common(scope)
            if common.get("context"):
                return [{"type": "memory", "summary": common["context"][:500], "layer": "common"}]
        return []
    # 把 compact 内容包装成旧格式（简化）
    entries: list[dict] = []
    for slice_name in ("boot", "context", "pending"):
        text = mem.get(slice_name, "")
        if text:
            entries.append({
                "type": "memory",
                "layer": slice_name,
                "summary": text[:500],
                "entity_type": scope.entity_type,
                "entity_id": scope.entity_id,
            })
    return entries
