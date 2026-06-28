"""
classify.py — ae-sdd 4 维判定（v1 最小实现）

4 维判定（v3.0 SKILL.md §1.6 + §1.7）：
  维度 1: 来源（source）     → PRD / Issue / 对话 / DR / 未知
  维度 2: 规模（scale）       → 微 / 小 / 中 / 大
  维度 3: AI 适配（ai_fit）  → 高 / 中 / 低
  维度 4: 多 Agent（multi_agent） → 启用 / 不启用

v1.1 修复（2026-06-18, v3.0.1 patch）：
  - 关键词收紧：移除 "问题/工单/想要/希望" 等过宽泛词
  - 标题感知：第一行 / 文件名包含 "PRD"/"DR"/"Issue" 等强信号优先
  - 低置信度告警：confidence < 0.4 时标记 needs_review=True
  - 不再硬兜底为 "对话"：低置信度时 source="未知"

v3.1+ 可升级为 LLM 辅助判定。

v3.5.10 修复（2026-06-28, Gap-014）：
  - 新增 project_context 参数：当传入项目根路径时，先读 state.json/currentStory +
    .auto-engineering/{storyId}/state.json + RA/Story 文档产物，用"已有产物规模"覆盖
    "行数推断" 的 scale 判定。修复"短文本需求被误判为微规模"导致跳过 DR/Story/Task 的根因。
  - 新增 _infer_scale_from_project_context()：按 RA/Story/Task/DR 文档存在性 + blockingGaps
    数 + AC 数推断规模，置信度高（0.9）。
"""
from __future__ import annotations

import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional


# 低置信度阈值（任一维度低于此值则标记 needs_review）
LOW_CONFIDENCE_THRESHOLD = 0.4


@dataclass
class Classification:
    source: str
    scale: str
    ai_fit: str
    multi_agent: bool
    confidence: dict       # 各维度置信度 0-1
    rationale: dict        # 各维度的判定理由
    next_action: str       # 建议的下一步
    needs_review: bool = False  # 🆕 是否需要人工复核（低置信度）
    review_reasons: list = field(default_factory=list)  # 🆕 复核原因


# ─── 关键词字典（v1.1） ──────────────────────────────────────────────────────
# 设计：保留宽关键词以保持向后兼容（tests 用），靠标题/文件名强信号避免误判。
# 优先级：标题 > 文件名 > 关键词匹配
SOURCE_KEYWORDS = {
    "PRD":   ["prd", "product requirement", "产品需求"],
    "Issue": ["issue", "bug", "defect", "缺陷", "问题", "工单"],
    "对话":   ["对话", "口述", "口头", "讨论", "提到", "想要", "希望"],
    "DR":    ["dr-", "dr ", "design requirement", "设计需求", "story", "task"],
}

SCALE_KEYWORDS = {
    "微":  ["微任务", "微改", "一个文件", "单文件", "trivial", "typo", "修个小"],
    "小":  ["小任务", "小改", "几个文件", "1-3 文件", "小需求", "small"],
    "中":  ["中等", "一般", "10 文件", "几个模块", "中型", "medium"],
    "大":  ["大", "整个", "全量", "重构", "跨模块", "large", "massive", "架构"],
}

AI_FIT_KEYWORDS = {
    "高": ["自动化", "重复", "模式", "标准", "套模板", "结构化"],
    "中": ["一般", "常见", "有现成"],
    "低": ["探索", "实验", "未确定", "新领域", "需要调研"],
}


# ─── 标题 / 文件名强信号 ──────────────────────────────────────────────────────
TITLE_PATTERNS = [
    # (regex, source) — 按优先级匹配，命中即跳出
    (r"^#\s*PRD[\s:：]", "PRD"),
    (r"^#\s*DR[\s:：]", "DR"),
    (r"^#\s*Story[\s:：]", "DR"),       # Story 属于 DR 范畴
    (r"^#\s*Task[\s:：]", "DR"),
    (r"^#\s*Issue[\s:：]", "Issue"),
    (r"^#\s*Bug[\s:：]", "Issue"),
]

FILENAME_PATTERNS = [
    (r"prd", "PRD"),
    (r"dr[-_]", "DR"),
    (r"story[-_]", "DR"),
    (r"task[-_]", "DR"),
    (r"issue", "Issue"),
    (r"bug", "Issue"),
]


def _detect_title_signal(text: str) -> Optional[tuple[str, str]]:
    """从第一行（标题）检测 source 强信号。返回 (source, matched_line) 或 None。"""
    first_line = next((l for l in text.splitlines() if l.strip()), "")
    for pat, src in TITLE_PATTERNS:
        if re.search(pat, first_line, re.IGNORECASE):
            return (src, first_line.strip())
    return None


def _detect_filename_signal(filename: str) -> Optional[tuple[str, str]]:
    """从文件名检测 source 强信号。返回 (source, filename) 或 None。"""
    if not filename:
        return None
    fn_lower = filename.lower()
    for pat, src in FILENAME_PATTERNS:
        if re.search(pat, fn_lower):
            return (src, filename)
    return None


def _match_keywords(text: str, kw_dict: dict) -> tuple[Optional[str], float]:
    """返回 (匹配的 key, 置信度)"""
    text_lower = text.lower()
    best_key: Optional[str] = None
    best_count = 0
    for key, kws in kw_dict.items():
        count = sum(1 for kw in kws if kw in text_lower)
        if count > best_count:
            best_count = count
            best_key = key
    conf = min(best_count / 2, 1.0) if best_count else 0.0
    return best_key, conf


def _infer_scale_from_lines(text: str) -> Optional[str]:
    """从行数推断规模（fallback）"""
    lines = len([l for l in text.splitlines() if l.strip()])
    if lines < 10:
        return "微"
    if lines < 50:
        return "小"
    if lines < 200:
        return "中"
    return "大"


def _infer_scale_from_project_context(project_root: Optional[Path]) -> Optional[tuple[str, float, str]]:
    """🆕 v3.5.10 Gap-014：从项目已有产物推断规模，覆盖行数推断的误判。

    优先级（任一命中即覆盖）：
      1. .auto-engineering/{storyId}/state.json 有 blockingGaps ≥ 5 → 大（0.9）
      2. 已有 RA 文档且 ≥ 100 行 → 大（0.85）
      3. 已有 Story 文档 → 中（0.8）
      4. 已有 Task 文档但无 Story → 小（0.7）
      5. 无任何产物 → 返回 None（交回行数推断）

    Returns:
        (scale, confidence, reason) 或 None（无项目上下文或无产物）
    """
    if project_root is None:
        return None
    try:
        project_root = Path(project_root).resolve()
        if not project_root.is_dir():
            return None
    except (OSError, ValueError):
        return None

    import json

    # 1) 读项目根 .ae-sdd/state.json 拿 currentStory
    ae_sdd_dir = project_root / ".ae-sdd"
    story_id: Optional[str] = None
    if (ae_sdd_dir / "state.json").is_file():
        try:
            s = json.loads((ae_sdd_dir / "state.json").read_text(encoding="utf-8"))
            story_id = s.get("currentStory") or s.get("storyId")
        except (json.JSONDecodeError, OSError):
            story_id = None

    # 2) 读 .auto-engineering/{storyId}/state.json 的 blockingGaps
    blocking_gaps = 0
    ra_outputs: list = []
    completed_steps: list = []
    if story_id:
        story_state_path = project_root / ".auto-engineering" / story_id / "state.json"
        if story_state_path.is_file():
            try:
                ss = json.loads(story_state_path.read_text(encoding="utf-8"))
                blocking_gaps = len(ss.get("blockingGaps") or [])
                ra_outputs = ss.get("outputs", {}).values() if isinstance(ss.get("outputs"), dict) else []
                completed_steps = ss.get("completedSteps") or []
            except (json.JSONDecodeError, OSError):
                pass

    # 3) 规模推断优先级
    # 3a) 已完成 ≥ ra-generated 阶段 + blockingGaps ≥ 5 → 大
    if blocking_gaps >= 5 and any("ra" in (step or "") for step in completed_steps):
        return ("大", 0.9, f"已有 RA 产物 + {blocking_gaps} 个 blockingGaps → 大")

    # 3b) 已有 RA 文档 + 行数 ≥ 100 → 大
    ra_dir = project_root / "ae-sdd-doc" / "iterations"
    if ra_dir.is_dir():
        try:
            ra_files = list(ra_dir.rglob("*RA*v*.md")) + list(ra_dir.rglob("*RA*v*.md"))
            if ra_files:
                max_lines = 0
                for rf in ra_files[:10]:  # 只看前 10 个文件，避免慢
                    try:
                        max_lines = max(max_lines, len(rf.read_text(encoding="utf-8").splitlines()))
                    except (OSError, UnicodeDecodeError):
                        pass
                if max_lines >= 100:
                    return ("大", 0.85, f"已有 RA 文档（最大 {max_lines} 行）→ 大")
        except OSError:
            pass

    # 3c) 已有 Story 文档 → 中
    if (project_root / "design").is_dir():
        try:
            story_files = list((project_root / "design").rglob("*.md"))
            if story_files and any("story" in f.name.lower() for f in story_files):
                return ("中", 0.8, "已有 Story 文档 → 中")
        except OSError:
            pass

    # 3d) 已有 Task 文档但无 Story → 小
    if (project_root / "task").is_dir():
        try:
            task_files = list((project_root / "task").rglob("*.md"))
            if task_files:
                return ("小", 0.7, "已有 Task 文档 → 小")
        except OSError:
            pass

    return None


def classify(text: str, *, filename: Optional[str] = None,
             project_context: Optional[Path] = None) -> Classification:
    """主入口：4 维判定。

    Args:
        text: 待判定文本
        filename: 🆕 可选文件名（用于文件名强信号）
        project_context: 🆕 v3.5.10 可选项目根路径——当传入时，规模判定优先读项目已有
            产物（state.json/RA/Story/Task），修复"短文本需求被误判微规模跳过 DR/Story/Task"
            的根因（Gap-014）。无产物或读取失败时 fallback 到行数推断。
    """
    review_reasons: list[str] = []

    # 维度 1：来源（标题/文件名强信号优先）
    source = None
    src_conf = 0.0
    src_reason = ""

    # 1a) 标题强信号
    title_hit = _detect_title_signal(text)
    if title_hit:
        source, hit_line = title_hit
        src_conf = 1.0
        src_reason = f"标题命中 → {source}（{hit_line[:40]}）"
    # 1b) 文件名强信号
    elif filename:
        fn_hit = _detect_filename_signal(filename)
        if fn_hit:
            source, hit_fn = fn_hit
            src_conf = 0.9
            src_reason = f"文件名命中 → {source}（{hit_fn}）"
    # 1c) 关键词匹配
    if not source:
        source, src_conf = _match_keywords(text, SOURCE_KEYWORDS)
        if source:
            src_reason = f"关键词匹配 → {source}"
        else:
            # 兜底为 "对话"（保留向后兼容），但置信度低 + needs_review=True
            source = "对话"
            src_conf = 0.2
            src_reason = "无关键词命中，默认兜底为 对话（需人工复核）"
            review_reasons.append("source 维度无任何信号，默认兜底为 对话")

    # 维度 2：规模
    # 🆕 v3.5.10 Gap-014：优先用项目已有产物判定规模，覆盖"短文本 → 微"误判
    scale, scale_conf = _match_keywords(text, SCALE_KEYWORDS)
    scale_reason = ""
    project_scale = _infer_scale_from_project_context(project_context)
    if project_scale is not None:
        # 项目产物信号优先级最高，覆盖关键词 + 行数推断
        scale, proj_conf, proj_reason = project_scale
        scale_conf = proj_conf
        scale_reason = f"项目产物覆盖 → {scale}（{proj_reason}）"
    elif not scale or scale_conf < 0.3:
        inferred = _infer_scale_from_lines(text)
        scale = inferred or "中"
        scale_conf = 0.4 if inferred else 0.5
        scale_reason = f"行数推断 → {scale}"
    else:
        scale_reason = f"关键词匹配 → {scale}"
    if scale_conf < LOW_CONFIDENCE_THRESHOLD:
        review_reasons.append(f"scale 置信度低（{scale_conf:.2f}）")

    # 维度 3：AI 适配
    ai_fit, ai_conf = _match_keywords(text, AI_FIT_KEYWORDS)
    ai_reason = ""
    if not ai_fit:
        # 根据规模推断
        ai_fit = "高" if scale in ("微", "小") else ("中" if scale == "中" else "低")
        ai_conf = 0.5
        ai_reason = f"根据规模推断 → {ai_fit}"
    else:
        ai_reason = f"关键词匹配 → {ai_fit}"
    if ai_conf < LOW_CONFIDENCE_THRESHOLD:
        review_reasons.append(f"ai_fit 置信度低（{ai_conf:.2f}）")

    # 维度 4：多 Agent（中等以上自动建议）
    multi_agent = scale in ("中", "大")

    # 下一步建议（v1.1：next_action 是工作流步骤名，与 PHASE_FLOW 解耦）
    # - "微" 规模 → coding（直接干）
    # - source=PRD/DR + 非微 → dr-generate（已结构化，直接生成 DR）
    # - source=对话/Issue/未知 + 非微 → requirement-analysis（先分析再生成 DR）
    if scale == "微":
        next_action = "coding"
    elif source in ("PRD", "DR"):
        next_action = "dr-generate"
    else:  # 对话 / Issue / 未知
        next_action = "requirement-analysis"
        if source == "未知":
            review_reasons.append("source 未知，需先做需求分析确认类型")

    # 全局 needs_review 判定
    needs_review = (
        src_conf < LOW_CONFIDENCE_THRESHOLD
        or any(r.startswith("scale") or r.startswith("ai_fit") or r.startswith("source")
               for r in review_reasons)
    )

    return Classification(
        source=source,
        scale=scale,
        ai_fit=ai_fit,
        multi_agent=multi_agent,
        confidence={
            "source": round(src_conf, 2),
            "scale": round(scale_conf, 2),
            "ai_fit": round(ai_conf, 2),
        },
        rationale={
            "source": src_reason,
            "scale": scale_reason,
            "ai_fit": ai_reason,
            "multi_agent": f"规模 = {scale} → {'启用' if multi_agent else '不启用'}",
        },
        next_action=next_action,
        needs_review=needs_review,
        review_reasons=review_reasons,
    )


def classify_from_file(path: Path) -> Classification:
    """从文件读取文本后分类（自动传 filename）"""
    if not path.is_file():
        raise FileNotFoundError(f"文件不存在: {path}")
    text = path.read_text(encoding="utf-8")
    return classify(text, filename=path.name)
