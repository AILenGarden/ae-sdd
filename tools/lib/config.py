"""
config.py — ae-sdd 项目实例配置加载（自动化开关）

读取 `.ae-sdd/config.yaml` 的 `automation` 段，合并母版默认值，对外暴露便捷查询函数。

设计原则：
- 母版默认值集中在本模块（AUTOMATION_DEFAULTS），避免硬编码散落 init.py / CLI / 门禁。
- 业务仓 config.yaml 可覆盖默认值（Layer4 覆盖母版默认）。
- 解析复用 paths.read_config 的简易解析器，但本模块额外支持数组字段
  （automatedReviewPoints: [1, 1.5, 2, 2.5, 4, 5]），read_config 原样返回字符串。

🆕 2026-07-02 v3.8.0：新增自动化开关（默认关闭；开启后 6 个人工审核点改走
Tier 3 多 reviewer 联审共识，实现 ae-sdd 全自动化：输入 → 结果）。
"""
from __future__ import annotations

from pathlib import Path
from typing import Optional

from lib import paths


# ─── 母版默认值（SSOT）──────────────────────────────────────────────────────
AUTOMATION_DEFAULTS: dict = {
    # 总开关：false=现状(每审核点等用户✅) / true=全自动化(审核点走联审共识)
    "enabled": False,
    # 联审强度：开启后统一强制 Tier 3（业务/架构/第三方视角三审交叉）
    "reviewerTier": 3,
    # 开工前信息预收集：扫输入材料+资产，列清单让用户一次补齐
    "preflightInfoCollection": True,
    # 阻断出口：联审 3 轮矫正未决时
    #   pause = state.phase=paused 等用户介入
    #   fail = 直接标记失败
    "onConsensusStall": "pause",
    # 审核点白名单（默认全部 6 个走联审；可配子集只让某些点自动化）
    # 合法值：1, 1.5, 2, 2.5, 4, 5
    "automatedReviewPoints": [1, 1.5, 2, 2.5, 4, 5],
    # 开启时间戳（审计用，AI 不得自行改；由 ae-sdd automation enable 写入）
    "enabledAt": "",
}

# 合法的审核点编号（与 SKILL.md §⏱️ 6 审核点一致，3/5 在 PRD 收尾非 review 节点）
VALID_REVIEW_POINTS = (1, 1.5, 2, 2.5, 4, 5)


def _parse_review_points(raw) -> list:
    """解析 automatedReviewPoints 字段，容忍字符串/列表两种形态。

    config.yaml 经 paths.read_config 解析后，`[1, 1.5, 2, 2.5, 4, 5]` 会保留为
    原始字符串 "[1, 1.5, 2, 2.5, 4, 5]"；手工构造的 dict 可能已是 list。
    """
    if isinstance(raw, list):
        return [float(x) for x in raw if x is not None]
    if isinstance(raw, str):
        s = raw.strip().strip("[]").strip()
        if not s:
            return list(AUTOMATION_DEFAULTS["automatedReviewPoints"])
        out = []
        for tok in s.split(","):
            tok = tok.strip().strip('"').strip("'")
            if not tok:
                continue
            try:
                out.append(float(tok))
            except ValueError:
                continue
        return out
    return list(AUTOMATION_DEFAULTS["automatedReviewPoints"])


def _to_bool(val, default: bool) -> bool:
    if isinstance(val, bool):
        return val
    if isinstance(val, str):
        return val.strip().lower() in ("true", "yes", "1", "on")
    return default


def _to_int(val, default: int) -> int:
    try:
        return int(val)
    except (TypeError, ValueError):
        return default


def load_automation_config(ade_sdd: Optional[Path] = None) -> dict:
    """加载 automation 段并合并默认值。

    Args:
        ade_sdd: `.ae-sdd/` 目录路径；None 时自动定位（向上查 5 级）。

    Returns:
        合并后的 automation 配置 dict，字段与 AUTOMATION_DEFAULTS 一致。
        定位不到 .ae-sdd/ 或 config.yaml 无 automation 段 → 返回默认值（关）。
    """
    merged = {k: (v.copy() if isinstance(v, list) else v)
              for k, v in AUTOMATION_DEFAULTS.items()}

    if ade_sdd is None:
        ade_sdd = paths.locate_project_ae_sdd()
    if ade_sdd is None:
        return merged

    cfg = paths.read_config(ade_sdd)
    auto = cfg.get("automation") if isinstance(cfg, dict) else None
    if not isinstance(auto, dict):
        return merged

    if "enabled" in auto:
        merged["enabled"] = _to_bool(auto["enabled"], merged["enabled"])
    if "reviewerTier" in auto:
        merged["reviewerTier"] = _to_int(auto["reviewerTier"], merged["reviewerTier"])
    if "preflightInfoCollection" in auto:
        merged["preflightInfoCollection"] = _to_bool(
            auto["preflightInfoCollection"], merged["preflightInfoCollection"])
    if "onConsensusStall" in auto and isinstance(auto["onConsensusStall"], str):
        v = auto["onConsensusStall"].strip().strip('"').strip("'")
        if v in ("pause", "fail"):
            merged["onConsensusStall"] = v
    if "automatedReviewPoints" in auto:
        pts = _parse_review_points(auto["automatedReviewPoints"])
        # 过滤非法编号
        merged["automatedReviewPoints"] = [p for p in pts if p in VALID_REVIEW_POINTS]
    if "enabledAt" in auto and isinstance(auto["enabledAt"], str):
        merged["enabledAt"] = auto["enabledAt"].strip().strip('"').strip("'")

    return merged


# ─── 便捷查询函数 ───────────────────────────────────────────────────────────
def is_automation_enabled(ade_sdd: Optional[Path] = None) -> bool:
    """自动化模式是否开启。"""
    return bool(load_automation_config(ade_sdd)["enabled"])


def get_reviewer_tier(ade_sdd: Optional[Path] = None) -> int:
    """自动化模式下的强制 reviewer Tier（默认 3）。"""
    return int(load_automation_config(ade_sdd)["reviewerTier"])


def get_automated_points(ade_sdd: Optional[Path] = None) -> list:
    """走自动联审的审核点编号列表。"""
    return load_automation_config(ade_sdd)["automatedReviewPoints"]


def is_point_automated(point: float, ade_sdd: Optional[Path] = None) -> bool:
    """指定审核点是否在自动化白名单内（且自动化已开启）。"""
    if not is_automation_enabled(ade_sdd):
        return False
    return point in get_automated_points(ade_sdd)


def get_stall_policy(ade_sdd: Optional[Path] = None) -> str:
    """联审停滞时的出口策略：'pause' 或 'fail'。"""
    return load_automation_config(ade_sdd)["onConsensusStall"]
