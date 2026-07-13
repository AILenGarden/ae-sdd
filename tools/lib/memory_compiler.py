"""
memory_compiler.py - compile source contexts into compact memory slices.

🆕 v3.10.3: memory 从"5层原文索引"重构为"业务实体树+编译文档容器"。
本模块负责把源上下文（DR/Story/约束/模板，从 document-storage 读取的原文）
编译成 3 个 compact slice（boot/context/pending）+ manifest，写入 memory 对应
实体目录。

设计原则（与 compile_skill_runtime.py 一致）：
  - 不做密文：compact 仍是可读 Markdown（表格/列表/JSON），不使用私有短码。
  - 高密度：去水词、表格化、引用符号化（DR §3.2 行120 而非复制原文）。
  - 确定性：同一输入编译两次结果完全一致（无时间戳/随机数）。

复用 compile_skill_runtime.py 的 _table/_stable_json 手法，但不 import 编译器
（编译器是构建脚本，memory 是运行时 lib，依赖方向不应反过来）。
"""
from __future__ import annotations

import hashlib
import json
from typing import Any


# --- compact slice renderers ---

def _escape_cell(value: Any) -> str:
    return str(value).replace("|", "\\|").replace("\n", " ").strip()


def _table(headers: list[str], rows: list[tuple[Any, ...]]) -> str:
    lines = [
        "| " + " | ".join(_escape_cell(h) for h in headers) + " |",
        "| " + " | ".join("---" for _ in headers) + " |",
    ]
    for row in rows:
        lines.append("| " + " | ".join(_escape_cell(c) for c in row) + " |")
    return "\n".join(lines)


def _stable_json(value: Any) -> str:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def _sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def render_boot_compact(
    *,
    entity_type: str,
    entity_id: str,
    series_chain: list[str],
    current_series: str,
    next_step: str,
    deliverables: list[dict[str, str]],
) -> str:
    """boot.compact.md: 锚点/当前系列/下一步/产物路径。"""
    deliverable_rows = [
        (d.get("name", ""), d.get("path", ""), d.get("status", ""))
        for d in deliverables
    ] or [("-", "-", "-")]
    return (
        f"# Memory Boot Compact - {entity_type}/{entity_id}\n\n"
        f"- entity_type: {entity_type}\n"
        f"- entity_id: {entity_id}\n"
        f"- current_series: {current_series}\n"
        f"- next_step: {next_step}\n"
        f"- deterministic: true\n\n"
        "## Series Chain\n\n"
        + " -> ".join(series_chain)
        + "\n\n## Deliverables\n\n"
        + _table(["name", "path", "status"], deliverable_rows)
        + "\n"
    )


def render_context_compact(
    *,
    dr_anchors: list[dict[str, str]],
    story_acs: list[dict[str, str]],
    constraints: list[str],
    api_contracts: list[dict[str, str]],
    data_models: list[dict[str, str]],
    asset_refs: list[str],
) -> str:
    """context.compact.md: 关键决策/约束/AC/接口契约/数据模型（高密度表格）。"""
    sections: list[str] = ["# Memory Context Compact\n"]

    if dr_anchors:
        sections.append("## DR Anchors\n")
        sections.append(_table(
            ["section", "line", "summary"],
            [(a.get("section", ""), a.get("line", ""), a.get("summary", "")) for a in dr_anchors],
        ))
        sections.append("")

    if story_acs:
        sections.append("## Story Acceptance Criteria\n")
        sections.append(_table(
            ["id", "description", "status"],
            [(ac.get("id", ""), ac.get("description", ""), ac.get("status", "")) for ac in story_acs],
        ))
        sections.append("")

    if constraints:
        sections.append("## Constraints\n")
        for c in constraints:
            sections.append(f"- {c}")
        sections.append("")

    if api_contracts:
        sections.append("## API Contracts\n")
        sections.append(_table(
            ["name", "method", "path", "request", "response"],
            [
                (
                    api.get("name", ""),
                    api.get("method", ""),
                    api.get("path", ""),
                    api.get("request", ""),
                    api.get("response", ""),
                )
                for api in api_contracts
            ],
        ))
        sections.append("")

    if data_models:
        sections.append("## Data Models\n")
        sections.append(_table(
            ["table", "fields", "notes"],
            [(dm.get("table", ""), dm.get("fields", ""), dm.get("notes", "")) for dm in data_models],
        ))
        sections.append("")

    if asset_refs:
        sections.append("## Asset References\n")
        for ref in asset_refs:
            sections.append(f"- `{ref}`")
        sections.append("")

    if len(sections) <= 1:
        sections.append("(no context extracted yet)\n")

    return "\n".join(sections)


def render_pending_compact(
    *,
    pending_items: list[dict[str, str]],
    failure_history: list[dict[str, str]],
    correction_counts: dict[str, int],
    review_loop_status: str,
) -> str:
    """pending.compact.md: 待决项/存疑项/失败历史/矫正计数/reviewLoop状态。"""
    sections: list[str] = ["# Memory Pending Compact\n"]

    sections.append(f"## Review Loop Status\n\n{review_loop_status or '(not started)'}\n")

    if pending_items:
        sections.append("## Pending Items\n")
        sections.append(_table(
            ["id", "description", "owner", "status"],
            [
                (
                    p.get("id", ""),
                    p.get("description", ""),
                    p.get("owner", ""),
                    p.get("status", ""),
                )
                for p in pending_items
            ],
        ))
        sections.append("")

    if failure_history:
        sections.append("## Failure History\n")
        sections.append(_table(
            ["round", "issue", "action"],
            [(f.get("round", ""), f.get("issue", ""), f.get("action", "")) for f in failure_history],
        ))
        sections.append("")

    if correction_counts:
        sections.append("## Correction Counts\n")
        sections.append(_table(
            ["phase", "count"],
            [(k, v) for k, v in sorted(correction_counts.items())],
        ))
        sections.append("")

    if len(sections) <= 2:
        sections.append("(no pending items)\n")

    return "\n".join(sections)


# --- common extraction ---

# common 层大小硬限制（字符数）。common 必须轻，不可臃肿。
COMMON_MAX_CHARS = 2048

# 约束关键字：含这些词的行视为"项目级可复用约束"，可提取到 common。
_CONSTRAINT_KEYWORDS = (
    "BigDecimal", "禁止", "必须", "幂等", "大事务", "SQL拼接", "敏感数据",
    "分布式", "硬编码", "跨层调用", "空catch", "循环内", "重复代码",
    "SOLID", "DRY", "KISS", "迪米特", "失败优先", "安全左移",
)


def extract_common(source_contexts: dict[str, str]) -> str:
    """从源上下文提取项目级可复用约束，写入 common/context.compact.md。

    Args:
        source_contexts: {context_name: content_text} 字典，如
            {"constraints": "...", "standards": "...", "assets": "..."}

    Returns:
        common context.compact.md 文本。严格限制 <= COMMON_MAX_CHARS 字符。
        超出时截断并追加告警。
    """
    constraint_lines: list[str] = []
    seen: set[str] = set()

    for ctx_name, content in source_contexts.items():
        if not content:
            continue
        for line in content.splitlines():
            stripped = line.strip()
            if not stripped:
                continue
            # 只提取含约束关键字的行（去重）
            if any(kw in stripped for kw in _CONSTRAINT_KEYWORDS):
                if stripped not in seen:
                    seen.add(stripped)
                    constraint_lines.append(stripped)

    if not constraint_lines:
        return "# Common Context Compact\n\n(no reusable constraints extracted)\n"

    body = "# Common Context Compact\n\n## Reusable Constraints\n\n"
    for line in constraint_lines:
        body += f"- {line}\n"

    if len(body) > COMMON_MAX_CHARS:
        # 截断到限制，追加告警
        warning = f"\n\n⚠️ common truncated: {len(body)} > {COMMON_MAX_CHARS} chars. Review source contexts for over-extraction.\n"
        body = body[: COMMON_MAX_CHARS - len(warning)] + warning

    return body


# --- manifest ---

def render_manifest(
    *,
    entity_type: str,
    entity_id: str,
    source_hashes: dict[str, str],
    boot_sha: str,
    context_sha: str,
    pending_sha: str,
) -> dict[str, Any]:
    """生成 manifest.json（校验用：source hash + slice hash + fingerprint）。"""
    fingerprint_payload = {
        "entity_type": entity_type,
        "entity_id": entity_id,
        "source_hashes": source_hashes,
        "boot_sha256": boot_sha,
        "context_sha256": context_sha,
        "pending_sha256": pending_sha,
    }
    fingerprint = _sha256_text(_stable_json(fingerprint_payload))
    return {
        "schema": "ae-sdd-memory/v1",
        "entity_type": entity_type,
        "entity_id": entity_id,
        "deterministic": True,
        "fingerprint": fingerprint,
        "source_hashes": source_hashes,
        "slices": {
            "boot": {"path": "boot.compact.md", "sha256": boot_sha},
            "context": {"path": "context.compact.md", "sha256": context_sha},
            "pending": {"path": "pending.compact.md", "sha256": pending_sha},
        },
    }


# --- top-level compile ---

def compile_source_to_memory(
    *,
    entity_type: str,
    entity_id: str,
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
) -> dict[str, str]:
    """读源上下文 -> 编译成 3 个 compact slice + manifest -> 返回内容字典。

    Args:
        entity_type: prd/dr/story/testcase/coding/common
        entity_id: 业务实体 ID（如 STORY-001-BE）
        source_contexts: {context_name: content_text}，用于计算 source hash + 提取 common
        其余参数：boot/context/pending slice 的结构化内容

    Returns:
        {
            "boot.compact.md": str,
            "context.compact.md": str,
            "pending.compact.md": str,
            "manifest.json": str (JSON text),
        }
    """
    series_chain = series_chain or []
    deliverables = deliverables or []
    dr_anchors = dr_anchors or []
    story_acs = story_acs or []
    constraints = constraints or []
    api_contracts = api_contracts or []
    data_models = data_models or []
    asset_refs = asset_refs or []
    pending_items = pending_items or []
    failure_history = failure_history or []
    correction_counts = correction_counts or {}

    # 计算 source hash
    source_hashes = {
        name: _sha256_text(content)
        for name, content in source_contexts.items()
        if content
    }

    # 渲染 3 个 slice
    boot_text = render_boot_compact(
        entity_type=entity_type,
        entity_id=entity_id,
        series_chain=series_chain,
        current_series=current_series,
        next_step=next_step,
        deliverables=deliverables,
    )
    context_text = render_context_compact(
        dr_anchors=dr_anchors,
        story_acs=story_acs,
        constraints=constraints,
        api_contracts=api_contracts,
        data_models=data_models,
        asset_refs=asset_refs,
    )
    pending_text = render_pending_compact(
        pending_items=pending_items,
        failure_history=failure_history,
        correction_counts=correction_counts,
        review_loop_status=review_loop_status,
    )

    # 计算 slice hash
    boot_sha = _sha256_text(boot_text)
    context_sha = _sha256_text(context_text)
    pending_sha = _sha256_text(pending_text)

    # 生成 manifest
    manifest = render_manifest(
        entity_type=entity_type,
        entity_id=entity_id,
        source_hashes=source_hashes,
        boot_sha=boot_sha,
        context_sha=context_sha,
        pending_sha=pending_sha,
    )

    return {
        "boot.compact.md": boot_text,
        "context.compact.md": context_text,
        "pending.compact.md": pending_text,
        "manifest.json": json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
    }
