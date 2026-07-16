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
  - 低置信度兜底为 "对话"（保留向后兼容）+ confidence < 0.4 标记 needs_review=True

🟡 LLM 升级路径状态（2026-07-03 核实，B4）：
  原注释"v3.1+ 可升级为 LLM 辅助判定"为悬空承诺，至 v3.8.1 未兑现亦未撤销。
  v3.5.10（Gap-014）与 v3.5.15 两度大改均继续走规则路线：
    - v3.5.10 加 project_context 参数，用"已有产物规模"覆盖"行数推断"绕开语义判定
    - v3.5.15 加 entry_node 字段，仍是规则匹配
  Gap-014 暴露的"短文本误判微规模"本是 LLM 强用例，但选择用规则绕开。
  设计文档 ae-sdd-design.md §2 对路由的描述为纯规则，与本实现一致；
  LLM 升级是否仍为演进方向待评估，不再作版本号承诺。

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
    entry_node: Optional[str] = None  # 🆕 v3.5.15 入口节点语义（FlowNode.value，如 BUG/CONFIG/PRD）


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
    🆕 v3.10.0 Route 下移重分级：大=DR入口、中=Story入口、小=CodingPlan入口、微=无文档。

    优先级（任一命中即覆盖）：
      1. .auto-engineering/{storyId}/state.json 有 blockingGaps ≥ 5 -> 大（0.9）
      2. 已有 RA 文档且 ≥ 100 行 -> 大（0.85）
      3. 已有 Story 文档 -> 中（0.8）（v3.10.0：有Story = 中任务入口）
      4. 已有 TestCase 但无 Story -> 小（0.7）（v3.10.0：有TestCase = 小任务 CodingPlan 入口）
      5. 无任何产物 -> 返回 None（交回行数推断）

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

    # 1) 只通过任务级 resolver 拿 activeStory/currentStory
    ae_sdd_dir = project_root / ".ae-sdd"
    story_id: Optional[str] = None
    resolved_state_path: Optional[Path] = None
    if ae_sdd_dir.is_dir():
        try:
            from lib import state as state_mod
            from lib import work_item_context
            resolved = work_item_context.resolve_default_state(ae_sdd_dir)
            s = resolved.data
            resolved_state_path = resolved.path
            story_id = state_mod.get_active_story(s) or s.get("storyId") or resolved.key
        except Exception:
            story_id = None

    # 2) 读 work-item state.json 的 blockingGaps（兼容 R6 与 legacy 目录）
    blocking_gaps = 0
    ra_outputs: list = []
    completed_steps: list = []
    if story_id:
        story_state_path = resolved_state_path or project_root / ".auto-engineering" / story_id / "state.json"
        try:
            from lib import paths as paths_mod
            story_state_path = (
                paths_mod.find_work_item_state_path(ae_sdd_dir, story_id)
                or story_state_path
            )
        except Exception:
            pass
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

    # 3c) 已有 Story 文档 -> 中（v3.10.0：已有Story = 从Story系列入 = 中任务）
    if (project_root / "design").is_dir():
        try:
            story_files = list((project_root / "design").rglob("*.md"))
            if story_files and any("story" in f.name.lower() for f in story_files):
                return ("中", 0.8, "已有 Story 文档 -> 中（从 Story 系列入）")
        except OSError:
            pass

    # 3d) 已有 TestCase 但无 Story -> 小（v3.10.0：有TestCase = CodingPlan 入口 = 小任务）
    if (project_root / "design").is_dir():
        try:
            tc_files = list((project_root / "design").rglob("*testcase*")) + list((project_root / "design").rglob("*TestCase*"))
            if tc_files:
                return ("小", 0.7, "已有 TestCase 文档 -> 小（从 CodingPlan 系列入）")
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
    # v3.12 三核心文档模型：
    # - "微"/"小" 规模 -> story-generate（Story-lite，随后 compact executionPlan）
    # - source=DR + 非微小 -> dr-generate（大任务 DR 入口）
    # - source=对话/Issue/未知 + 非微小 -> requirement-analysis（先分析再生成 DR）
    if scale in ("微", "小"):
        next_action = "story-generate"
    elif source in ("PRD", "DR"):
        next_action = "dr-generate"
    else:  # 对话 / Issue
        next_action = "requirement-analysis"

    # 全局 needs_review 判定
    needs_review = (
        src_conf < LOW_CONFIDENCE_THRESHOLD
        or any(r.startswith("scale") or r.startswith("ai_fit") or r.startswith("source")
               for r in review_reasons)
    )

    # 🆕 v3.5.15 入口节点语义推断（entry_node，FlowNode.value）
    # BUG/配置类 → scale="微" + entry_node=BUG/CONFIG（复用微链，不单独开链）
    # PRD/DR/Issue/对话 → 按 source 映射 FlowNode
    # 🆕 v3.10.2 micro 意图分流：优化/审查类 → scale="微" + entry_node=OPTIMIZE/CODE_REVIEW
    #   消歧优先级：self-update 上下文（ae-sdd/SKILL/流程）> 代码上下文。
    #   "优化 ae-sdd" → 不进 micro（self-update 由 SKILL.md 路由表第①步接管）
    #   "优化这部分实现" + 代码上下文 → OPTIMIZE 微链
    # 🆕 v3.11.6 micro 意图分流第三支：仅调整已有设计文档排版/格式、语义不变
    #   → scale="微" + entry_node=DOC_FORMAT。要求文档上下文 + 非内容变更信号，
    #   消歧优先级同上：self-update 上下文 > 文档上下文；内容变更信号 > 格式关键词。
    entry_node: Optional[str] = None
    text_lower = text.lower()
    if any(kw in text_lower for kw in ("bug", "缺陷", "故障", "修复", "fix")):
        entry_node = "BUG"
        if scale != "微":
            scale = "微"  # BUG 修复强制微链
    elif any(kw in text_lower for kw in ("配置", "config", "改个常量", "改个枚举")):
        entry_node = "CONFIG"
        if scale != "微":
            scale = "微"
    elif (any(kw in text_lower for kw in _OPTIMIZE_KEYWORDS)
          and _is_code_context(text_lower)
          and not _has_selfupdate_context(text_lower)):
        # 优化/重构/改进 + 代码上下文 + 非self-update上下文 → micro-optimize
        # 要求代码上下文：避免"大重构整个架构"被误降为微（重构同时是大规模关键词）。
        # "优化这部分实现"命中"实现"，"重构这个方法"命中"方法"。
        entry_node = "OPTIMIZE"
        if scale != "微":
            scale = "微"
    elif (any(kw in text_lower for kw in _REVIEW_KEYWORDS)
          and not _has_selfupdate_context(text_lower)):
        # 审查/CodeReview/评审代码 → micro-review
        # 审查天然是代码语境，不额外要求代码上下文词
        entry_node = "CODE_REVIEW"
        if scale != "微":
            scale = "微"
    elif (any(kw in text_lower for kw in _DOC_FORMAT_KEYWORDS)
          and _is_doc_context(text_lower)
          and not _has_doc_format_selfupdate_context(text_lower)
          and not _has_content_change_signal(text_lower)):
        # 🆕 v3.11.6 仅调整已有设计文档排版/格式、语义不变 → micro-doc-format
        # 要求文档上下文（Story/DR/PRD/TestCase/.md/模板等词）：避免"格式化代码"落入本分支。
        # 要求非内容变更信号（新增字段/新增接口/需求变更等）：避免语义变更被误判为纯格式任务。
        entry_node = "DOC_FORMAT"
        if scale != "微":
            scale = "微"
    elif source == "PRD":
        entry_node = "PRD"
    elif source == "Issue":
        entry_node = "RA"  # Issue 需求走 RA
    elif source in ("对话", "未知"):
        entry_node = "RA"

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
        entry_node=entry_node,
    )


def classify_from_file(path: Path) -> Classification:
    """从文件读取文本后分类（自动传 filename）"""
    if not path.is_file():
        raise FileNotFoundError(f"文件不存在: {path}")
    text = path.read_text(encoding="utf-8")
    return classify(text, filename=path.name)


# ─── 🆕 v3.9.0 嵌套状态模型：自动 state 匹配/新建（R7）──────────────────────────
# 治本缺口：v3.8.x 前 classify() 只判规格+入口系列，不扫现有 state、不做特征匹配，
#   导致新任务不自动开 state，镜像死锁旧任务。
#
# v3.9.0 新增 match_state()：在 /ae-sdd 路由时自动
#   1. 分析需求特征（提取 PRD/DR/Story ID + 判定是否 Bug/改 Story）
#   2. 扫描现有嵌套 state，按 R2 向上归入优先级匹配
#   3. 找不到则建议以当前主体为顶层新建
#
# 匹配优先级（R2/R4/R5/R7）：
#   1. R4: is_bug_fix and not modifies_story → create_flat（微任务独立 state）
#   2. R5: story_ids 命中现有 state.storyStates → relocate + reset_substate（若已过 coding）
#   3. R2 向上归入: story_ids 所属 DR 命中现有 state.drState → 归入该 state 的 storyStates
#   4. R2 向上归入: dr_id 命中现有 state.prdState（DR 属于某 PRD）→ 归入该 state 的 drState
#   5. R7: 无匹配 → create_nested（以 top_node 为顶层）


# Story ID 正则（如 STORY-003-BE / STORY-003）
_STORY_ID_RE = re.compile(r"STORY[-_]?(\d+)(?:[-_]?\w+)?", re.IGNORECASE)
# DR ID 正则（如 DR-CS / DR-001）
_DR_ID_RE = re.compile(r"\bDR[-_]?([A-Za-z0-9-]+)", re.IGNORECASE)
# PRD ID 正则（如 PRD-IM-CS / PRD-001）
_PRD_ID_RE = re.compile(r"\bPRD[-_]?([A-Za-z0-9-]+)", re.IGNORECASE)

# Bug/微任务关键词（与 classify() 主逻辑一致）
_BUG_KEYWORDS = ("bug", "缺陷", "故障", "修复", "fix", "typo", "配置", "config", "改个常量", "改个枚举")

# 🆕 v3.10.2 micro 意图分流：优化/审查关键词与上下文消歧
# 消歧优先级：self-update 上下文 > 代码上下文（"优化 ae-sdd 的代码"应判 self-update）
# 注意："重构" 同时在 SCALE_KEYWORDS["大"]，必须搭配代码上下文才进 micro-optimize，
#   否则"大重构整个架构"会被误降为微（与 test_classify 回归冲突）。
_OPTIMIZE_KEYWORDS = ("优化", "重构", "改进")
_REVIEW_KEYWORDS = ("codereview", "code review", "cr报告", "cr 报告", "审查", "评审代码", "代码审查")
# self-update 上下文词：命中则"优化"指向 ae-sdd 自身，不进 micro-optimize
_SELFUPDATE_CONTEXT_KEYWORDS = ("ae-sdd", "ae_sdd", "auto-engineering", "skill", "流程", "门禁", "runtime", "compile_skill", "dist/", "tools/lib")
# 代码上下文词：表明"优化/改进"的对象是具体项目代码而非 ae-sdd 自身/系统架构。
# 刻意排除"模块/架构/逻辑"等宏观词——这些词同时出现在大规模重构语境，
# 避免误把"大重构整个模块"降为微。只收"指向具体代码片段"的词。
_CODE_CONTEXT_KEYWORDS = (
    "代码", "实现", "这段", "这部分", "这个文件", "该方法", "这个方法", "这个类", "函数",
    "源码", "service", "controller", "mapper", "entity", "bean", "repository",
    ".java", ".py", ".ts", ".js", ".go", ".cpp", ".c", ".rs",
)

# 🆕 v3.11.6 micro 意图分流第三支：仅调整已有设计文档排版/格式，语义不变
# 关键词收窄为"格式/排版"专属表达，避免与 _OPTIMIZE_KEYWORDS（优化/重构/改进）重叠。
_DOC_FORMAT_KEYWORDS = ("调整格式", "格式调整", "排版", "格式化", "套模板", "改格式", "统一格式")
# 文档上下文词：表明操作对象是设计文档而非代码，避免"格式化代码"误入本分支。
_DOC_CONTEXT_KEYWORDS = (
    "文档", "story", "dr", "prd", "testcase", "test case", "用例文档",
    ".md", "模板", "章节", "md 文件",
)
# 内容变更信号词：命中则说明本次改动涉及语义/字段/流程变化，不是纯格式任务，
# 即使同时出现"格式"类词也不得判 DOC_FORMAT（防止"顺带改格式+改字段"被误判为零语义变更）。
_CONTENT_CHANGE_KEYWORDS = (
    "新增字段", "删除字段", "新增接口", "新增章节", "修改语义", "变更需求",
    "调整逻辑", "补充需求", "新增ac", "新增 ac", "字段变更", "接口变更",
)

# 🆕 v3.11.6 引用型前缀：用户在"援引 ae-sdd 定义的标准/模板"而非"修改 ae-sdd 自身"时使用。
# 例："根据 ae-sdd 的 Story 模板格式调整文档"——ae-sdd 是被引用的标准来源，不是修改目标。
# 命中该前缀时，DOC_FORMAT 分支不把裸 "ae-sdd"/"ae_sdd" 词计入 self-update 上下文
#（其余 self-update 关键词如 skill/流程/门禁 仍照常生效，见 _has_doc_format_selfupdate_context）。
_AE_SDD_REFERENCE_PREFIX_RE = re.compile(
    r"(根据|按照|按|依据|参照|遵循|套用)\s*(ae-sdd|ae_sdd)", re.IGNORECASE
)


def _has_selfupdate_context(text_lower: str) -> bool:
    """🆕 v3.10.2 检测文本是否指向 ae-sdd/SKILL/流程（self-update 上下文）。

    消歧关键：`优化` 同时能匹配 micro-optimize 和 self-update。当文本含
    ae-sdd/SKILL/流程 等词时，"优化"指向 ae-sdd 自身，应走 self-update 路由，
    不进 micro-optimize。
    """
    return any(kw in text_lower for kw in _SELFUPDATE_CONTEXT_KEYWORDS)


def _is_code_context(text_lower: str) -> bool:
    """🆕 v3.10.2 检测文本是否指向项目代码（代码上下文）。

    用于区分"优化代码"（micro-optimize）vs"优化 ae-sdd"（self-update）。
    命中代码/实现/这段代码/service/mapper 等词 → 代码上下文。
    """
    return any(kw in text_lower for kw in _CODE_CONTEXT_KEYWORDS)


def _is_doc_context(text_lower: str) -> bool:
    """🆕 v3.11.6 检测文本是否指向设计文档（文档上下文）。

    用于区分"调整文档格式"（micro-doc-format）vs"格式化代码"（不进本分支）。
    命中 文档/Story/DR/PRD/TestCase/.md/模板 等词 → 文档上下文。
    """
    return any(kw in text_lower for kw in _DOC_CONTEXT_KEYWORDS)


def _has_content_change_signal(text_lower: str) -> bool:
    """🆕 v3.11.6 检测文本是否含内容/语义变更信号。

    命中则说明本次改动不是纯格式调整（如"调整格式并新增字段"），
    应回退到正常 Story Update 流程而非 micro-doc-format 轻量路径。
    """
    return any(kw in text_lower for kw in _CONTENT_CHANGE_KEYWORDS)


def _has_doc_format_selfupdate_context(text_lower: str) -> bool:
    """🆕 v3.11.6 DOC_FORMAT 专用 self-update 上下文检测（比通用版更精确）。

    通用 `_has_selfupdate_context` 对裸 "ae-sdd" 词做子串匹配，会误伤"根据 ae-sdd
    的 Story 模板格式调整文档"这类『引用 ae-sdd 标准』的表达（ae-sdd 是被引用的
    标准来源，不是修改目标）。本函数在通用检测命中前，先排除 _AE_SDD_REFERENCE_PREFIX_RE
    命中的引用型前缀场景；skill/流程/门禁/runtime 等其余 self-update 词不受影响，
    命中仍视为真正的 self-update 上下文（如"优化 ae-sdd 的 skill 路由逻辑"）。
    """
    if _AE_SDD_REFERENCE_PREFIX_RE.search(text_lower):
        # 引用型前缀命中：裸 "ae-sdd"/"ae_sdd" 不计入信号，仅看其余 self-update 词
        remaining_keywords = tuple(
            kw for kw in _SELFUPDATE_CONTEXT_KEYWORDS if kw not in ("ae-sdd", "ae_sdd")
        )
        return any(kw in text_lower for kw in remaining_keywords)
    return _has_selfupdate_context(text_lower)


@dataclass
class RequirementFeatures:
    """需求特征提取结果（R7 match_state 输入）。"""
    top_node: str                      # 当前工作主体 PRD/DR/STORY
    prd_id: Optional[str] = None       # 溯源 PRD（从需求文本提取）
    dr_id: Optional[str] = None        # 溯源 DR
    story_ids: list = field(default_factory=list)  # 涉及的 Story ID
    is_bug_fix: bool = False           # R4 判定
    modifies_story: bool = False       # 是否改动已存在 Story
    confidence: float = 0.5            # 特征提取置信度
    reasons: list = field(default_factory=list)    # 提取理由


@dataclass
class StateMatchResult:
    """match_state 输出：建议的 state 动作。

    action 取值：
      - "create_flat"      R4 微任务新建独立扁平 state
      - "create_nested"    R7 无匹配，以当前主体为顶层新建嵌套 state
      - "relocate"         R5 改已管理 Story，重定位回所属 state
      - "absorb_into_prd"  R2 DR/Story 归入已存在的 PRD state
      - "absorb_into_dr"   R2 Story 归入已存在的 DR state
    """
    action: str
    target_state_path: Optional[Path] = None  # relocate/absorb 时指向已存在 state
    target_state_data: Optional[dict] = None  # 配套 state dict
    story_to_reset: Optional[str] = None      # R5 需重置的 Story ID
    naming: Optional[str] = None              # R6 命名（create_nested 时）
    entry_node: Optional[str] = None          # create_nested 时的 entryNode
    reasons: list = field(default_factory=list)


def extract_requirement_features(text: str,
                                  project_root: Optional[Path] = None) -> RequirementFeatures:
    """R7: 从需求文本提取特征（PRD/DR/Story ID + Bug 判定 + 改 Story 判定）。

    Args:
        text: 用户需求文本
        project_root: 项目根（用于判定 Story 是否已存在 → modifies_story）

    Returns:
        RequirementFeatures
    """
    reasons: list[str] = []
    story_ids = list(dict.fromkeys(_STORY_ID_RE.findall(text)))  # 去重保序
    # 补全 STORY- 前缀（正则只捕获数字部分时）
    story_ids = [s if s.upper().startswith("STORY") else f"STORY-{s}" for s in story_ids]
    # 若正则匹配了完整 ID（含 -BE 等），findall 只取数字组，这里重抓完整
    full_story_matches = re.findall(r"STORY[-_]?\d+(?:[-_]?[A-Za-z]+)?", text, re.IGNORECASE)
    story_ids = list(dict.fromkeys(s.upper() for s in full_story_matches))

    dr_match = _DR_ID_RE.search(text)
    dr_id = f"DR-{dr_match.group(1)}" if dr_match else None
    prd_match = _PRD_ID_RE.search(text)
    prd_id = f"PRD-{prd_match.group(1)}" if prd_match else None

    # R4 Bug 判定
    text_lower = text.lower()
    is_bug_fix = any(kw in text_lower for kw in _BUG_KEYWORDS)

    # modifies_story 判定：有 Story ID 且不是纯 Bug
    modifies_story = bool(story_ids) and not (is_bug_fix and "story" not in text_lower)

    # top_node 判定优先级：有 Story → STORY；有 DR → DR；有 PRD → PRD；Bug → TASK
    if is_bug_fix and not modifies_story:
        top_node = "TASK"
        reasons.append("Bug/微任务不改 Story → top_node=TASK")
    elif story_ids:
        top_node = "STORY"
        reasons.append(f"含 Story ID {story_ids} → top_node=STORY")
    elif dr_id:
        top_node = "DR"
        reasons.append(f"含 DR ID {dr_id} → top_node=DR")
    elif prd_id:
        top_node = "PRD"
        reasons.append(f"含 PRD ID {prd_id} → top_node=PRD")
    else:
        top_node = "STORY"  # 默认按 Story 处理（最常见）
        reasons.append("无明确 ID，默认 top_node=STORY")

    confidence = 0.9 if (story_ids or dr_id or prd_id) else 0.4

    return RequirementFeatures(
        top_node=top_node,
        prd_id=prd_id,
        dr_id=dr_id,
        story_ids=story_ids,
        is_bug_fix=is_bug_fix,
        modifies_story=modifies_story,
        confidence=confidence,
        reasons=reasons,
    )


def match_state(project_root: Path, features: RequirementFeatures) -> StateMatchResult:
    """R7: 分析需求特征 → 扫描现有 state → 匹配/新建判定。

    匹配优先级见模块 docstring（R4 > R5 > R2向上归入 > R7新建）。

    Args:
        project_root: 项目根路径
        features: extract_requirement_features 提取的特征

    Returns:
        StateMatchResult（含 action + target_state_path/naming 等）
    """
    from lib import paths as paths_mod

    ade_sdd = paths_mod.locate_project_ae_sdd(project_root)
    if ade_sdd is None:
        # 无 .ae-sdd 目录：仍生成命名，但 action 标记需新建
        try:
            naming_features: dict = {}
            if features.top_node == "PRD":
                naming_features["prd_feature"] = (features.prd_id or "UNKNOWN").replace("PRD-", "", 1)
            elif features.top_node == "DR":
                naming_features["dr_feature"] = (features.dr_id or "UNKNOWN").replace("DR-", "", 1)
            elif features.top_node == "STORY":
                naming_features["story_ids"] = features.story_ids or ["STORY-000"]
            naming = paths_mod.build_state_machine_name(features.top_node, naming_features) if features.top_node in ("PRD", "DR", "STORY") else None
        except ValueError:
            naming = None
        return StateMatchResult(
            action="create_flat" if features.top_node == "TASK" else "create_nested",
            entry_node=features.top_node if features.top_node != "TASK" else None,
            naming=naming,
            reasons=["未找到 .ae-sdd 目录，建议新建 state"],
        )

    reasons: list[str] = []

    # 1) R4: Bug/微任务不改 Story → create_flat
    if features.is_bug_fix and not features.modifies_story:
        reasons.append("R4: Bug/微任务不改 Story → create_flat")
        return StateMatchResult(action="create_flat", reasons=reasons)

    # 2) R5: story_ids 命中现有嵌套 state → relocate + reset_substate
    for sid in features.story_ids:
        hit = paths_mod.find_nested_state_by_story_id(ade_sdd, sid)
        if hit:
            state_path, state_data = hit
            # 判定是否需要 R5 重置：该 Story phase 已过 story-generated（即下游已动过）
            from lib import state as state_mod
            sub = state_mod.get_story_substate(state_data, sid)
            phase_now = sub.get("phase", "initialized") if sub else "initialized"
            _STORY_DOWNSTREAM = {"task-generated", "task-reviewed", "coding-process",
                                 "coding", "test-running", "code-reviewed", "completed"}
            need_reset = phase_now in _STORY_DOWNSTREAM
            reasons.append(
                f"R5: Story {sid} 命中 state {state_data.get('stateMachineId')} "
                f"(phase={phase_now}) → relocate"
                + (f" + reset_substate（下游已动）" if need_reset else "")
            )
            return StateMatchResult(
                action="relocate",
                target_state_path=state_path,
                target_state_data=state_data,
                story_to_reset=sid if need_reset else None,
                reasons=reasons,
            )

    # 3) R2 向上归入: story_ids 所属 DR 命中现有 state.drState
    if features.story_ids and features.dr_id:
        # 🆕 v3.9.3 P-V: 验证父级 DR 文档存在 + 关联性
        design_dir = paths_mod.project_design_dir(paths_mod.project_root(ade_sdd))
        for sid in features.story_ids:
            ok, reason = paths_mod.verify_parent_claim("DR", features.dr_id, design_dir, child_id=sid)
            if not ok and reason == "relation_mismatch":
                reasons.append(
                    f"R2 阻断：DR {features.dr_id} 文档存在但未列出 {sid} → relation_mismatch"
                )
                # 关联性不对 → 阻断（用户要求）
                from dataclasses import dataclass, field as _field
                return StateMatchResult(
                    action="create_nested",
                    reasons=reasons + [f"⚠️ 父级 DR 关联性不对，请去 design/DR-{features.dr_id.replace('DR-', '', 1)}-*.md 补 {sid}"],
                )
            if not ok and reason == "doc_not_found":
                reasons.append(f"R2 父级 DR 文档不存在 → 视为无父级")
                features.dr_id = None
                break
        if features.dr_id:
            hit = paths_mod.find_nested_state_by_dr_id(ade_sdd, features.dr_id)
            if hit:
                state_path, state_data = hit
                reasons.append(
                    f"R2: Story 所属 DR {features.dr_id} 命中 state "
                    f"{state_data.get('stateMachineId')} → absorb_into_dr"
                )
                return StateMatchResult(
                    action="absorb_into_dr",
                    target_state_path=state_path,
                    target_state_data=state_data,
                    reasons=reasons,
                )

    # 4) R2 向上归入: dr_id 所属 PRD 命中现有 state.prdState
    if features.dr_id and features.prd_id:
        # 🆕 v3.9.3 P-V: 验证父级 PRD 文档存在
        design_dir = paths_mod.project_design_dir(paths_mod.project_root(ade_sdd))
        ok, reason = paths_mod.verify_parent_claim("PRD", features.prd_id, design_dir, child_id=features.dr_id)
        if not ok and reason == "relation_mismatch":
            reasons.append(
                f"R2 阻断：PRD {features.prd_id} 文档存在但未列出 {features.dr_id}"
            )
            return StateMatchResult(
                action="create_nested",
                reasons=reasons + [f"⚠️ 父级 PRD 关联性不对"],
            )
        if not ok and reason == "doc_not_found":
            reasons.append(f"R2 父级 PRD 文档不存在 → 视为无父级")
            features.prd_id = None
        else:
            hit = paths_mod.find_nested_state_by_prd_id(ade_sdd, features.prd_id)
            if hit:
                state_path, state_data = hit
                reasons.append(
                    f"R2: DR {features.dr_id} 所属 PRD {features.prd_id} 命中 state "
                    f"{state_data.get('stateMachineId')} → absorb_into_prd"
                )
                return StateMatchResult(
                    action="absorb_into_prd",
                    target_state_path=state_path,
                    target_state_data=state_data,
                    reasons=reasons,
                )

    # 5) R7: 无匹配 → create_nested（以 top_node 为顶层）
    # 若 top_node 是 TASK（Bug），改走 create_flat
    if features.top_node == "TASK":
        reasons.append("R7: 无匹配 + top_node=TASK → create_flat")
        return StateMatchResult(action="create_flat", reasons=reasons)

    # R6 命名
    naming_features: dict = {}
    if features.top_node == "PRD":
        naming_features["prd_feature"] = (features.prd_id or "UNKNOWN").replace("PRD-", "", 1)
    elif features.top_node == "DR":
        naming_features["dr_feature"] = (features.dr_id or "UNKNOWN").replace("DR-", "", 1)
    elif features.top_node == "STORY":
        naming_features["story_ids"] = features.story_ids or ["STORY-000"]

    try:
        naming = paths_mod.build_state_machine_name(features.top_node, naming_features)
    except ValueError as e:
        naming = f"{features.top_node}-UNKNOWN"
        reasons.append(f"命名生成失败: {e}")

    reasons.append(f"R7: 无匹配 → create_nested (entryNode={features.top_node}, name={naming})")
    return StateMatchResult(
        action="create_nested",
        entry_node=features.top_node,
        naming=naming,
        reasons=reasons,
    )
