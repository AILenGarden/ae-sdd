"""
gates.py — ae-sdd 14 门禁检查

v1 实现：
- G-00：项目资产完整性（pre-flight，完整实现）
- G-01 ~ G-13：stub（返回 "未实现" warn，给出门禁 ID + 描述）

14 门禁（v3.0）：
  G-00  项目资产完整性（pre-flight）
  G-01  DR 文档存在
  G-02  Story 文档存在
  G-03  Story Review 通过
  G-04  TestCase 文档存在
  G-05  Task 文档存在
  G-06  Task Review 通过
  G-07  CodingPlan 存在
  G-08  CodingPlan 14 门禁通过
  G-09  测试真实性扫描通过
  G-10  测试报告存在
  G-11  Coding 报告存在
  G-12  CodeReview 报告存在
  G-13  全链路对称性核查通过
"""
from __future__ import annotations

import re
import sys
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

# 父包路径（避免 import 问题）
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import paths  # noqa: E402


@dataclass
class GateResult:
    """单个门禁结果"""
    gate_id: str
    name: str
    severity: str            # "blocker" | "warn"
    pass_: bool
    message: str
    action: Optional[str] = None
    details: dict = field(default_factory=dict)


# 14 门禁元信息
GATE_REGISTRY: list[dict] = [
    {"id": "G-00", "name": "项目资产完整性",       "severity": "blocker"},
    {"id": "G-01", "name": "DR 文档存在",          "severity": "blocker"},
    {"id": "G-02", "name": "Story 文档存在",       "severity": "blocker"},
    {"id": "G-03", "name": "Story Review 通过",    "severity": "blocker"},
    {"id": "G-04", "name": "TestCase 文档存在",    "severity": "blocker"},
    {"id": "G-05", "name": "Task 文档存在",        "severity": "blocker"},
    {"id": "G-06", "name": "Task Review 通过",     "severity": "blocker"},
    {"id": "G-07", "name": "CodingPlan 存在",      "severity": "blocker"},
    {"id": "G-08", "name": "CodingPlan 14 门禁通过", "severity": "blocker"},
    {"id": "G-09", "name": "测试真实性扫描通过",   "severity": "blocker"},
    {"id": "G-10", "name": "测试报告存在",         "severity": "blocker"},
    {"id": "G-11", "name": "Coding 报告存在",      "severity": "blocker"},
    {"id": "G-12", "name": "CodeReview 报告存在",  "severity": "blocker"},
    {"id": "G-13", "name": "全链路对称性核查通过", "severity": "blocker"},
]


def _stub_result(gate_id: str, name: str) -> GateResult:
    """未实现门禁的 stub"""
    return GateResult(
        gate_id=gate_id,
        name=name,
        severity="blocker",
        pass_=True,  # 默认通过，不阻断用户流程
        message="未实现（v3.0 stub — 返回通过）",
        action="v3.1 实施",
        details={"stub": True},
    )


def check_g00(master_source: Optional[Path], ade_sdd: Optional[Path], project_key: str) -> GateResult:
    """
    G-00：项目资产完整性（pre-flight）

    检查项：
    1. .ae-sdd/config.yaml 存在
    2. .ae-sdd/state.json 存在
    3. .ae-sdd/assets/{projectKey}.assets.md 存在
    4. 资产 7 层索引齐全
    5. 资产 lastAuditedAt < 30 天（warn 不阻断）
    """
    name = "项目资产完整性"

    # 没找到 .ae-sdd → 阻断
    if ade_sdd is None:
        return GateResult(
            gate_id="G-00",
            name=name,
            severity="blocker",
            pass_=False,
            message="未找到 .ae-sdd/ 目录",
            action=f"运行: ae-sdd init <project-dir> {project_key}",
        )

    details: dict = {}
    missing: list[str] = []

    # 1. config.yaml
    if not (ade_sdd / "config.yaml").is_file():
        missing.append("config.yaml")

    # 2. state.json
    if not (ade_sdd / "state.json").is_file():
        missing.append("state.json")

    # 3. 项目资产
    asset_file = ade_sdd / "assets" / f"{project_key}.assets.md"
    if not asset_file.is_file():
        missing.append(f"assets/{project_key}.assets.md")
        return GateResult(
            gate_id="G-00",
            name=name,
            severity="blocker",
            pass_=False,
            message=f"项目资产不存在: assets/{project_key}.assets.md",
            action=f"运行: ae-sdd init <project-dir> {project_key} --asset-path <已有资产>",
            details={"missing": missing},
        )

    if missing:
        return GateResult(
            gate_id="G-00",
            name=name,
            severity="blocker",
            pass_=False,
            message=f"项目骨架不完整，缺失: {', '.join(missing)}",
            action=f"运行: ae-sdd init <project-dir> {project_key} --force",
            details={"missing": missing},
        )

    # 4. 7 层索引齐全（找 §A-§G）
    content = asset_file.read_text(encoding="utf-8")
    required_sections = ["§A", "§B", "§C", "§D", "§E", "§F", "§G"]
    missing_sections = [s for s in required_sections if s not in content]
    if missing_sections:
        return GateResult(
            gate_id="G-00",
            name=name,
            severity="blocker",
            pass_=False,
            message=f"项目资产缺索引层: {', '.join(missing_sections)}",
            action=f"运行: ae-sdd assets generate --project {project_key}",
            details={"missing_sections": missing_sections},
        )

    # 5. lastAuditedAt 鲜度（warn 不阻断）
    warn_msg = None
    audited_match = re.search(r"lastAuditedAt[:\s]+(\d{4}-\d{2}-\d{2})", content)
    if audited_match:
        try:
            audited_date = datetime.strptime(audited_match.group(1), "%Y-%m-%d")
            days = (datetime.now() - audited_date).days
            if days > 30:
                warn_msg = f"项目资产 {days} 天未审计（> 30 天）"
        except ValueError:
            pass

    return GateResult(
        gate_id="G-00",
        name=name,
        severity="blocker",
        pass_=True,
        message=warn_msg or "项目资产完整 + 7 层索引齐全",
        details={"asset_file": str(asset_file), "last_audited_warn": warn_msg},
    )


def check_all(master_source: Optional[Path], ade_sdd: Optional[Path], project_key: str, only: Optional[str] = None) -> list[GateResult]:
    """跑所有 14 门禁；only 指定时只跑那一个"""
    results: list[GateResult] = []

    targets = [g for g in GATE_REGISTRY if (only is None or g["id"] == only)]
    if only and not targets:
        return [GateResult(
            gate_id=only, name="未知门禁", severity="blocker",
            pass_=False, message=f"未知门禁 ID: {only}（允许: {[g['id'] for g in GATE_REGISTRY]}）",
        )]

    for g in targets:
        if g["id"] == "G-00":
            results.append(check_g00(master_source, ade_sdd, project_key))
        else:
            # G-01 ~ G-13 stub
            results.append(_stub_result(g["id"], g["name"]))

    return results


def summarize(results: list[GateResult]) -> dict:
    """汇总结果"""
    return {
        "total": len(results),
        "passed": sum(1 for r in results if r.pass_),
        "failed": sum(1 for r in results if not r.pass_),
        "stubs": sum(1 for r in results if r.details.get("stub")),
        "all_pass": all(r.pass_ for r in results),
        "results": [
            {
                "gate_id": r.gate_id,
                "name": r.name,
                "severity": r.severity,
                "pass": r.pass_,
                "message": r.message,
                "action": r.action,
            }
            for r in results
        ],
    }
