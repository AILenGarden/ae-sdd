"""
classify.py — ae-sdd 4 维判定（v1 最小实现）

4 维判定（v3.0 SKILL.md §1.6 + §1.7）：
  维度 1: 来源（source）     → PRD / Issue / 对话 / DR
  维度 2: 规模（scale）       → 微 / 小 / 中 / 大
  维度 3: AI 适配（ai_fit）  → 高 / 中 / 低
  维度 4: 多 Agent（multi_agent） → 启用 / 不启用

v1 简单实现：基于关键词匹配。
v3.1+ 可升级为 LLM 辅助判定。
"""
from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


@dataclass
class Classification:
    source: str
    scale: str
    ai_fit: str
    multi_agent: bool
    confidence: dict       # 各维度置信度 0-1
    rationale: dict        # 各维度的判定理由
    next_action: str       # 建议的下一步


# ─── 关键词字典（v1） ────────────────────────────────────────────────────────
SOURCE_KEYWORDS = {
    "PRD":   ["prd", "product requirement", "产品需求"],
    "Issue": ["issue", "bug", "defect", "缺陷", "问题", "工单"],
    "对话":   ["对话", "口述", "口头", "讨论", "提到", "想要", "希望"],
    "DR":    ["dr", "design requirement", "设计需求", "story", "task"],
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


def _match_keywords(text: str, kw_dict: dict) -> tuple[Optional[str], float]:
    """返回 (匹配的 key, 置信度)"""
    text_lower = text.lower()
    best_key: Optional[str] = None
    best_count = 0
    total = 0
    for key, kws in kw_dict.items():
        count = sum(1 for kw in kws if kw in text_lower)
        total += count
        if count > best_count:
            best_count = count
            best_key = key
    conf = min(best_count / 3, 1.0) if best_count else 0.0
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


def classify(text: str) -> Classification:
    """主入口：4 维判定"""
    # 维度 1：来源
    source, src_conf = _match_keywords(text, SOURCE_KEYWORDS)
    if not source:
        source = "对话"  # 默认
        src_conf = 0.2

    # 维度 2：规模
    scale, scale_conf = _match_keywords(text, SCALE_KEYWORDS)
    if not scale or scale_conf < 0.3:
        inferred = _infer_scale_from_lines(text)
        scale = inferred or "中"
        scale_conf = 0.4 if inferred else 0.5

    # 维度 3：AI 适配
    ai_fit, ai_conf = _match_keywords(text, AI_FIT_KEYWORDS)
    if not ai_fit:
        # 根据规模推断
        ai_fit = "高" if scale in ("微", "小") else ("中" if scale == "中" else "低")
        ai_conf = 0.5

    # 维度 4：多 Agent（中等以上自动建议）
    multi_agent = scale in ("中", "大")

    # 下一步建议
    next_action_map = {
        "PRD":   "requirement-analysis",
        "Issue": "requirement-analysis",
        "对话":   "requirement-analysis",
        "DR":    "dr-generate",
    }
    next_action = next_action_map.get(source, "requirement-analysis")
    if scale == "微":
        next_action = "coding"
    elif scale == "小":
        next_action = "task-generate"
    elif scale in ("中", "大"):
        next_action = "dr-generate"

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
            "source": f"匹配关键词 → {source}",
            "scale": f"匹配关键词 → {scale}" if scale_conf > 0.3 else f"行数推断 → {scale}",
            "ai_fit": f"匹配关键词 → {ai_fit}" if ai_conf > 0.5 else f"根据规模推断 → {ai_fit}",
            "multi_agent": f"规模 = {scale} → {'启用' if multi_agent else '不启用'}",
        },
        next_action=next_action,
    )


def classify_from_file(path: Path) -> Classification:
    """从文件读取文本后分类"""
    if not path.is_file():
        raise FileNotFoundError(f"文件不存在: {path}")
    text = path.read_text(encoding="utf-8")
    return classify(text)
