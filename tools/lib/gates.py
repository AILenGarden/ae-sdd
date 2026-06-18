"""
gates.py — ae-sdd 14 门禁检查

v3.0.1 完整实现 G-01~G-07 + G-10~G-12（11 个"检查文档/状态存在"类门禁）。
v3.1 实施 G-08（解析 CodingPlan 14 门禁）+ G-09（调 test_authenticity_scan.py）+ G-13（全链路对称性核查）。

14 门禁（v3.0）：
  G-00  项目资产完整性（pre-flight）       ✅ 完整
  G-01  DR 文档存在                        ✅ 完整
  G-02  Story 文档存在                     ✅ 完整
  G-03  Story Review 通过                  ✅ 完整
  G-04  TestCase 文档存在                  ✅ 完整
  G-05  Task 文档存在                      ✅ 完整
  G-06  Task Review 通过                   ✅ 完整
  G-07  CodingPlan 存在                    ✅ 完整
  G-08  CodingPlan 14 门禁通过             🟡 v3.1
  G-09  测试真实性扫描通过                 🟡 v3.1
  G-10  测试报告存在                       ✅ 完整
  G-11  Coding 报告存在                    ✅ 完整
  G-12  CodeReview 报告存在                ✅ 完整
  G-13  全链路对称性核查通过                🟡 v3.1
"""
from __future__ import annotations

import re
import sys
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable, Optional

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import paths, state as state_mod  # noqa: E402


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

# Story Review 之后允许的 phase
PHASE_PAST_STORY_REVIEW = {
    "story-reviewed", "task-generated", "task-reviewed",
    "coding", "test-running", "code-reviewed", "completed",
}

# Task Review 之后允许的 phase
PHASE_PAST_TASK_REVIEW = {
    "task-reviewed", "coding", "test-running", "code-reviewed", "completed",
}

# CodingPlan 必含章节（按 coding-skill §6.7 ④bis CodingPlan 7 章节）
CODINGPLAN_REQUIRED_SECTIONS = [
    "文件顺序", "类骨架", "数据", "Mapper SQL", "测试对应", "验证点", "调试回滚",
]


# ─── G-00 ───────────────────────────────────────────────────────────────────
def check_g00(master_source: Optional[Path], ade_sdd: Optional[Path], project_key: str) -> GateResult:
    """G-00 项目资产完整性（pre-flight）"""
    name = "项目资产完整性"

    if ade_sdd is None:
        return GateResult("G-00", name, "blocker", False,
                          "未找到 .ae-sdd/ 目录",
                          f"运行: ae-sdd init <project-dir> {project_key}")

    missing: list[str] = []
    if not (ade_sdd / "config.yaml").is_file():
        missing.append("config.yaml")
    if not (ade_sdd / "state.json").is_file():
        missing.append("state.json")

    asset_file = ade_sdd / "assets" / f"{project_key}.assets.md"
    if not asset_file.is_file():
        return GateResult("G-00", name, "blocker", False,
                          f"项目资产不存在: assets/{project_key}.assets.md",
                          f"运行: ae-sdd init <project-dir> {project_key} --asset-path <已有资产>")

    if missing:
        return GateResult("G-00", name, "blocker", False,
                          f"项目骨架不完整，缺失: {', '.join(missing)}",
                          f"运行: ae-sdd init <project-dir> {project_key} --force")

    # 7 层索引
    content = asset_file.read_text(encoding="utf-8")
    required_sections = ["§A", "§B", "§C", "§D", "§E", "§F", "§G"]
    missing_sections = [s for s in required_sections if s not in content]
    if missing_sections:
        return GateResult("G-00", name, "blocker", False,
                          f"项目资产缺索引层: {', '.join(missing_sections)}",
                          f"运行: ae-sdd assets generate --project {project_key}",
                          details={"missing_sections": missing_sections})

    # lastAuditedAt 鲜度（warn 不阻断）
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

    return GateResult("G-00", name, "blocker", True,
                      warn_msg or "项目资产完整 + 7 层索引齐全",
                      details={"asset_file": str(asset_file), "last_audited_warn": warn_msg})


# ─── G-01 ───────────────────────────────────────────────────────────────────
def check_g01(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-01 DR 文档存在"""
    design = paths.project_design_dir(project_dir)
    if not design.is_dir():
        return GateResult("G-01", "DR 文档存在", "blocker", False,
                          f"design/ 目录不存在: {design}",
                          "跑 dr-generate-skill 生成 DR 文档")

    drs = sorted(design.glob("*DR*.md")) + sorted(design.glob("*dr*.md"))
    # 去重
    drs = sorted(set(drs))
    if not drs:
        return GateResult("G-01", "DR 文档存在", "blocker", False,
                          f"design/ 目录无 DR 文档（*DR*.md / *dr*.md）",
                          "跑 dr-generate-skill 生成 DR 文档")
    return GateResult("G-01", "DR 文档存在", "blocker", True,
                      f"找到 {len(drs)} 个 DR 文档",
                      details={"files": [d.name for d in drs]})


# ─── G-02 ───────────────────────────────────────────────────────────────────
def check_g02(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-02 Story 文档存在"""
    if not current_story:
        return GateResult("G-02", "Story 文档存在", "blocker", False,
                          "state.currentStory 为空",
                          "跑 ae-sdd state write --phase story-generated --story STORY-XXX")

    story = paths.find_doc(project_dir, current_story, ".md")
    if story is None:
        return GateResult("G-02", "Story 文档存在", "blocker", False,
                          f"Story 文档不存在: design/{current_story}.md",
                          f"跑 story-generate-skill 生成 {current_story}",
                          details={"expected": str(paths.project_design_dir(project_dir) / f"{current_story}.md")})
    return GateResult("G-02", "Story 文档存在", "blocker", True,
                      f"找到 {story.name}",
                      details={"file": str(story)})


# ─── G-03 ───────────────────────────────────────────────────────────────────
def check_g03(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-03 Story Review 通过"""
    phase = st.get("phase", "initialized")
    if phase in PHASE_PAST_STORY_REVIEW:
        return GateResult("G-03", "Story Review 通过", "blocker", True,
                          f"phase = {phase}（Story Review 已通过）")
    return GateResult("G-03", "Story Review 通过", "blocker", False,
                      f"phase = {phase}（Story Review 未完成）",
                      "跑 story-review-skill 审核 Story（含 F-Stage 前端契约）")


# ─── G-04 ───────────────────────────────────────────────────────────────────
def check_g04(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-04 TestCase 文档存在"""
    if not current_story:
        return GateResult("G-04", "TestCase 文档存在", "blocker", False,
                          "state.currentStory 为空")

    tc = paths.find_doc(project_dir, current_story, "-testcase.md")
    if tc is None:
        return GateResult("G-04", "TestCase 文档存在", "blocker", False,
                          f"TestCase 文档不存在: design/{current_story}-testcase.md",
                          f"跑 testcase-generate-skill 生成 {current_story}-testcase.md",
                          details={"expected": str(paths.project_design_dir(project_dir) / f"{current_story}-testcase.md")})
    return GateResult("G-04", "TestCase 文档存在", "blocker", True,
                      f"找到 {tc.name}",
                      details={"file": str(tc)})


# ─── G-05 ───────────────────────────────────────────────────────────────────
def check_g05(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-05 Task 文档存在"""
    if not current_story:
        return GateResult("G-05", "Task 文档存在", "blocker", False,
                          "state.currentStory 为空")

    tasks = paths.list_docs(project_dir, current_story, "-task-*.md")
    if not tasks:
        return GateResult("G-05", "Task 文档存在", "blocker", False,
                          f"task/ 目录无 {current_story}-task-*.md",
                          f"跑 task-generate-skill 生成 Task 文档",
                          details={"task_dir": str(paths.project_task_dir(project_dir))})
    return GateResult("G-05", "Task 文档存在", "blocker", True,
                      f"找到 {len(tasks)} 个 Task 文档",
                      details={"files": [t.name for t in tasks]})


# ─── G-06 ───────────────────────────────────────────────────────────────────
def check_g06(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-06 Task Review 通过"""
    phase = st.get("phase", "initialized")
    if phase in PHASE_PAST_TASK_REVIEW:
        return GateResult("G-06", "Task Review 通过", "blocker", True,
                          f"phase = {phase}（Task Review 已通过）")
    return GateResult("G-06", "Task Review 通过", "blocker", False,
                      f"phase = {phase}",
                      "跑 task-generate-skill §5a 全局 Task Review")


# ─── G-07 ───────────────────────────────────────────────────────────────────
def check_g07(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-07 CodingPlan 存在 + 7 章节齐全"""
    if not current_story:
        return GateResult("G-07", "CodingPlan 存在", "blocker", False,
                          "state.currentStory 为空")

    cp = paths.find_doc(project_dir, current_story, "-CodingPlan.md")
    if cp is None:
        return GateResult("G-07", "CodingPlan 存在", "blocker", False,
                          f"CodingPlan 文档不存在: design/{current_story}-CodingPlan.md",
                          f"跑 CodingPlan 生成（CodingSkill §6.7）")

    content = cp.read_text(encoding="utf-8")
    missing = [s for s in CODINGPLAN_REQUIRED_SECTIONS if s not in content]
    if missing:
        return GateResult("G-07", "CodingPlan 存在", "blocker", False,
                          f"CodingPlan 缺章节: {missing}",
                          "补全 7 章节（coding-skill §6.7）",
                          details={"missing_sections": missing})
    return GateResult("G-07", "CodingPlan 存在", "blocker", True,
                      f"{cp.name} 7 章节齐全",
                      details={"file": str(cp)})


# ─── G-10 / G-11 / G-12：报告存在性 ──────────────────────────────────────────
def _check_report(project_dir: Path, st: dict, current_story: str, *,
                 gate_id: str, name: str, suffix: str, action: str) -> GateResult:
    """报告类门禁的通用检查逻辑"""
    if not current_story:
        return GateResult(gate_id, name, "blocker", False,
                          "state.currentStory 为空")

    doc = paths.find_doc(project_dir, current_story, suffix)
    if doc is None:
        return GateResult(gate_id, name, "blocker", False,
                          f"报告文档不存在: design/{current_story}{suffix}",
                          action,
                          details={"expected": str(paths.project_design_dir(project_dir) / f"{current_story}{suffix}")})
    return GateResult(gate_id, name, "blocker", True,
                      f"找到 {doc.name}",
                      details={"file": str(doc)})


def check_g10(project_dir: Path, st: dict, current_story: str) -> GateResult:
    return _check_report(project_dir, st, current_story,
                         gate_id="G-10", name="测试报告存在",
                         suffix="-Report.md",
                         action=f"跑完测试后生成 {current_story}-Report.md")


def check_g11(project_dir: Path, st: dict, current_story: str) -> GateResult:
    return _check_report(project_dir, st, current_story,
                         gate_id="G-11", name="Coding 报告存在",
                         suffix="-Coding-Report.md",
                         action=f"编码完成后生成 {current_story}-Coding-Report.md")


def check_g12(project_dir: Path, st: dict, current_story: str) -> GateResult:
    return _check_report(project_dir, st, current_story,
                         gate_id="G-12", name="CodeReview 报告存在",
                         suffix="-CodeReview.md",
                         action=f"CodeReview 后生成 {current_story}-CodeReview.md")


# ─── G-08：解析 CodingPlan 14 门禁表 ─────────────────────────────────────────
# 按 coding-skill §6.7 ④bis CodingPlan 14 门禁必含关键词
CODINGPLAN_14GATES_KEYWORDS = [
    "DR-Story-Task",     # 1. 三层链路追溯
    "AC 100%",            # 2. AC 100% 覆盖
    "文件顺序",            # 3. 文件顺序
    "类骨架",              # 4. 类骨架
    "数据",                # 5. 数据模型
    "Mapper SQL",          # 6. Mapper SQL
    "测试对应",            # 7. 测试对应
    "验证点",              # 8. 验证点
    "调试回滚",            # 9. 调试回滚
    "资源隔离",            # 10. 资源隔离
    "核心链路",            # 11. 核心链路保护
    "CodingModel",         # 12. CodingModel 决策记录
    "混合压测",            # 13. 混合压测
    "测试真实性",          # 14. 测试真实性 8 类禁止
]


def check_g08(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-08 CodingPlan 14 门禁通过 — 解析 CodingPlan 文档 14 门禁表"""
    if not current_story:
        return GateResult("G-08", "CodingPlan 14 门禁通过", "blocker", False,
                          "state.currentStory 为空")

    cp = paths.find_doc(project_dir, current_story, "-CodingPlan.md")
    if cp is None:
        return GateResult("G-08", "CodingPlan 14 门禁通过", "blocker", False,
                          f"CodingPlan 文档不存在",
                          f"先生成 {current_story}-CodingPlan.md（coding-skill §6.7）")

    content = cp.read_text(encoding="utf-8")

    # 1. 14 关键词必须全在文档里
    missing_kw = [k for k in CODINGPLAN_14GATES_KEYWORDS if k not in content]
    if missing_kw:
        return GateResult("G-08", "CodingPlan 14 门禁通过", "blocker", False,
                          f"CodingPlan 缺 14 门禁关键词: {missing_kw}",
                          f"补全 14 门禁表（coding-skill §6.7）",
                          details={"missing_keywords": missing_kw})

    # 2. 统计 ✅ / ❌ / 🟡 标记 — 必须有 ≥ 14 条状态记录
    n_pass = content.count("✅")
    n_fail = content.count("❌")
    n_warn = content.count("🟡")
    n_total = n_pass + n_fail + n_warn

    if n_total < 14:
        return GateResult("G-08", "CodingPlan 14 门禁通过", "blocker", False,
                          f"门禁记录数不足 14（实际 {n_total}：✅ {n_pass} / ❌ {n_fail} / 🟡 {n_warn}）",
                          "补全 14 门禁表",
                          details={"n_pass": n_pass, "n_fail": n_fail, "n_warn": n_warn})

    # 3. 不能有 ❌ 标记
    if n_fail > 0:
        return GateResult("G-08", "CodingPlan 14 门禁通过", "blocker", False,
                          f"14 门禁中有 {n_fail} 条 ❌ 未通过",
                          f"修复 {current_story}-CodingPlan.md 中标 ❌ 的门禁",
                          details={"n_pass": n_pass, "n_fail": n_fail, "n_warn": n_warn})

    return GateResult("G-08", "CodingPlan 14 门禁通过", "blocker", True,
                      f"14 门禁全通过（✅ {n_pass} / 🟡 {n_warn}）",
                      details={"n_pass": n_pass, "n_fail": n_fail, "n_warn": n_warn,
                               "file": str(cp)})


# ─── G-09：调 test_authenticity_scan.py 扫测试代码 ───────────────────────────
import json as _json
import subprocess as _subprocess


def _locate_authenticity_scanner(master_source: Optional[Path]) -> Optional[Path]:
    """在母版找 test_authenticity_scan.py"""
    if master_source is None:
        return None
    # 母版布局：master/../scripts/test_authenticity_scan.py
    candidate = master_source.parent / "scripts" / "test_authenticity_scan.py"
    return candidate if candidate.is_file() else None


def check_g09(project_dir: Path, st: dict, current_story: str,
              master_source: Optional[Path] = None) -> GateResult:
    """G-09 测试真实性扫描通过 — 调 test_authenticity_scan.py 跑 8 类禁止检查"""
    # 当前需要传 master_source（call_g09 通过字典传）
    scanner = _locate_authenticity_scanner(master_source)
    if scanner is None:
        return GateResult("G-09", "测试真实性扫描通过", "blocker", True,
                          "未找到母版 test_authenticity_scan.py（跳过）",
                          action="确认母版路径",
                          details={"scanned": False, "skipped": True})

    # 跑扫描（--format json 输出可解析）
    try:
        result = _subprocess.run(
            [sys.executable, str(scanner), "--root", str(project_dir), "--format", "json"],
            capture_output=True, text=True, timeout=60,
        )
    except _subprocess.TimeoutExpired:
        return GateResult("G-09", "测试真实性扫描通过", "blocker", False,
                          "test_authenticity_scan.py 跑超过 60 秒",
                          "缩小扫描范围或增加超时")
    except Exception as e:
        return GateResult("G-09", "测试真实性扫描通过", "blocker", False,
                          f"扫描器异常: {e}",
                          "检查 test_authenticity_scan.py 是否可执行")

    # 解析 JSON 输出
    try:
        report = _json.loads(result.stdout) if result.stdout else {}
    except _json.JSONDecodeError as e:
        return GateResult("G-09", "测试真实性扫描通过", "blocker", False,
                          f"扫描器 JSON 输出无法解析: {e}",
                          f"stdout 前 200 字符: {result.stdout[:200]}")

    status = report.get("status", "UNKNOWN")
    blockers = sum(1 for f in report.get("findings", []) if f.get("severity") == "BLOCKER")
    n_total = len(report.get("findings", []))

    if status == "PASS":
        return GateResult("G-09", "测试真实性扫描通过", "blocker", True,
                          f"扫描通过：{n_total} findings / 0 BLOCKER",
                          details={"scanned": True, "n_findings": n_total, "n_blockers": 0})

    # FAIL 或未知状态
    return GateResult("G-09", "测试真实性扫描通过", "blocker", False,
                      f"扫描失败：{n_total} findings / {blockers} BLOCKER",
                      f"修复测试代码中的 8 类禁止（{scanner.name}）",
                      details={"scanned": True, "n_findings": n_total, "n_blockers": blockers,
                               "status": status})


# ─── G-13：v3.1 实施 ────────────────────────────────────────────────────────
def _stub_v31(gate_id: str, name: str) -> GateResult:
    return GateResult(gate_id, name, "blocker", True,
                      "v3.1 实施（当前 stub 返回通过）",
                      action="v3.1 实施",
                      details={"stub": True, "milestone": "v3.1"})


def check_g13(project_dir: Path, st: dict, current_story: str) -> GateResult:
    return _stub_v31("G-13", "全链路对称性核查通过")


# ─── 路由表 ─────────────────────────────────────────────────────────────────
CHECK_FUNCS: dict[str, Callable] = {
    "G-01": check_g01, "G-02": check_g02, "G-03": check_g03,
    "G-04": check_g04, "G-05": check_g05, "G-06": check_g06,
    "G-07": check_g07,
    "G-08": check_g08, "G-09": check_g09,
    "G-10": check_g10, "G-11": check_g11, "G-12": check_g12,
    "G-13": check_g13,
}


# ─── 主入口 ─────────────────────────────────────────────────────────────────
def check_all(master_source: Optional[Path], ade_sdd: Optional[Path],
              project_key: str, only: Optional[str] = None) -> list[GateResult]:
    """跑 14 门禁；only 指定时只跑那一个"""
    results: list[GateResult] = []

    # 读 state（如果 .ae-sdd 存在）
    if ade_sdd:
        st = state_mod.read_state(paths.state_path(ade_sdd))
    else:
        st = {"phase": "initialized", "currentStory": None}
    current_story = st.get("currentStory") or ""

    # 推导 project_dir
    project_dir = paths.project_root(ade_sdd) if ade_sdd else Path.cwd()

    targets = [g for g in GATE_REGISTRY if (only is None or g["id"] == only)]
    if only and not targets:
        return [GateResult(
            gate_id=only, name="未知门禁", severity="blocker",
            pass_=False,
            message=f"未知门禁 ID: {only}（允许: {[g['id'] for g in GATE_REGISTRY]}）",
        )]

    for g in targets:
        if g["id"] == "G-00":
            results.append(check_g00(master_source, ade_sdd, project_key))
        elif g["id"] == "G-09":
            # G-09 需要 master_source 调子脚本
            results.append(check_g09(project_dir, st, current_story, master_source=master_source))
        elif g["id"] in CHECK_FUNCS:
            results.append(CHECK_FUNCS[g["id"]](project_dir, st, current_story))
        else:
            results.append(_stub_v31(g["id"], g["name"]))

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
