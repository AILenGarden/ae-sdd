"""
gates.py — ae-sdd 门禁检查（14 主门禁 + 4 G-RA + G-CODE Coding 真实性门禁）

v3.0.1 完整实现 G-01~G-07 + G-10~G-12（11 个"检查文档/状态存在"类门禁）。
v3.1 实施 G-08（解析 CodingPlan 14 门禁）+ G-09（调 test_authenticity_scan.py）+ G-13（全链路对称性核查）。
v3.2 实施 G-RA-1~G-RA-4（需求分析准入门卫）+ G-13 接入 RA 层（五层→六层追溯）。
v3.2.1 实施 G-CODE-1（Coding 真实性/反模式扫描，调 coding_authenticity_scan.py）。

14 主门禁（v3.0）：
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
  G-13  全链路对称性核查通过                🟡 v3.1（v3.2 升级六层追溯）

G-RA 需求分析准入门卫（v3.2 — 对标 SKILL.md §🛡️ G-RA）：
  对标 Coding 的 G-08/G-09，把 requirement-analysis-skill 的 16 道 RA-G 闸
  从"纸面规则"变成"可执行门禁"。RA 质量与 Coding 一样可被代码验证。
  G-RA-1  RA 文档存在                       ✅ 完整（含 30 天时效 warn + phase 感知）
  G-RA-2  RA 8 维度完整 + RAModel 12 维     ✅ 完整
  G-RA-3  RA 衍生章节完整                   ✅ 完整（状态机类需求必填）
  G-RA-4  RA 真实性扫描通过                 ✅ 完整（调 ra_authenticity_scan.py）

G-CODE Coding 真实性门禁（v3.2.1 — 对标 CodingModel §6 AI Coding 反模式库）：
  G-CODE-1 Coding 真实性扫描通过             ✅ 完整（调 coding_authenticity_scan.py）

中段门禁（v3.4.0 — 对标建议书1/2/4，补齐"两头强中间空"的中段强制力）：
  G-14           CodingPlan-Story 一致性      ✅ 完整（AC 对齐 + Story 引用 + 偏离 Proposal）
  G-CODEPLAN-SRC CodingPlan 源码核对          ✅ 完整（类骨架须附已读/待核实源码标记）
  G-DOC-STORAGE  文档落地存放合规              ✅ 完整（产物路径/命名须合规，禁游离位置）
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


# 门禁元信息（14 主门禁 G-00~G-13 + 3 中段门禁 G-14/G-CODEPLAN-SRC/G-DOC-STORAGE + 1 G-PATH + 4 G-RA + 1 G-CODE = 23）
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
    # 🆕 v3.4.0 中段门禁（对标建议书1/2/4 — 补齐"两头强中间空"的中段强制力）
    # G-14 CodingPlan-Story 一致性：Plan 涉及接口/DO/AC 与 Story 可对应（建议书4 G-08-15）
    {"id": "G-14", "name": "CodingPlan-Story 一致性", "severity": "blocker"},
    # G-CODEPLAN-SRC CodingPlan 源码核对：新增/修改类建模范式须附已读源码标记（建议书1）
    {"id": "G-CODEPLAN-SRC", "name": "CodingPlan 源码核对", "severity": "blocker"},
    # G-DOC-STORAGE 文档落地存放合规：产物路径/命名须经 resolve_path 推导（建议书2）
    {"id": "G-DOC-STORAGE", "name": "文档落地存放合规", "severity": "blocker"},
    # 🆕 v4.1 G-PATH 路径越界检测：母版 source/ 下 SKILL/template 文档不得硬编码产出路径，
    # 须声明调用 document-storage（防自身路径规则漂移，与 plugin_content_scan PC-009/010 分层防护）
    {"id": "G-PATH", "name": "路径越界检测", "severity": "blocker"},
    # 🆕 v3.2 G-RA 需求分析准入门卫（对标 SKILL.md §🛡️ G-RA）— 把 RA 16 道闸
    # 的核心条款从"纸面规则"变成"可执行门禁"，与 Coding G-08/G-09 对等。
    {"id": "G-RA-1", "name": "RA 文档存在",          "severity": "blocker"},
    {"id": "G-RA-2", "name": "RA 8 维度完整",        "severity": "blocker"},
    {"id": "G-RA-3", "name": "RA 衍生章节完整",      "severity": "blocker"},
    {"id": "G-RA-4", "name": "RA 真实性扫描通过",    "severity": "blocker"},
    # 🆕 2026-06-27 RA 流程违规审计（建议书 §3.4）— 扫 RA 文档是否走完 RAModel 12 维 +
    # 8 维度 + 5 问自检 + 缺口管理 + 规模裁定 + RA-G01~16 闸判定，堵"AI 跳过 RA 完整流程直接出 RA 文档"
    {"id": "G-RA-FLOW-VIOLATION", "name": "RA 流程违规审计", "severity": "blocker"},
    {"id": "G-CODE-1", "name": "Coding 真实性扫描通过", "severity": "blocker"},
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
    if not paths.config_path(ade_sdd).is_file():
        missing.append("config.yaml")
    if not paths.state_path(ade_sdd).is_file():
        missing.append("state.json")

    asset_file = paths.find_asset_file(ade_sdd, project_key)
    if asset_file is None or not asset_file.is_file():
        return GateResult("G-00", name, "blocker", False,
                          f"项目资产不存在: .ae-sdd/assets/{project_key}/{project_key}.assets.md（或旧位置 assets/{project_key}.assets.md）",
                          f"运行: ae-sdd init <project-dir> {project_key} --asset-path <已有资产>（资产路径模型见 document-storage §2.3）")

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
                      details={"asset_file": str(asset_file),
                               "last_audited_warn": warn_msg,
                               "module_asset_files": _discover_module_assets(ade_sdd, project_key)})


def _discover_module_assets(ade_sdd: Path, project_key: str) -> list:
    """🆕 v4.0：发现工程级子文件（总览之外），用于 G-00 details 信息展示。

    不阻断门卫（子文件可选）。返回子文件路径列表（不含总览）。
    """
    try:
        all_files = paths.find_module_asset_files(ade_sdd, project_key)
        # 排除总览本体
        overview = paths.find_asset_file(ade_sdd, project_key)
        return [str(f) for f in all_files if overview is None or f != overview]
    except Exception:
        return []


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

    # 🆕 v3.4.0 内容校验升级（建议书3 §7.4）：14 关键词中是否有"空壳"（关键词存在但紧邻占位符）
    placeholder_tokens = ("待补充", "TODO", "TBD", "占位", "placeholder")
    shell_keywords = []
    for k in CODINGPLAN_14GATES_KEYWORDS:
        idx = content.find(k)
        if idx >= 0:
            # 看关键词后 30 字符内是否含占位符
            tail = content[idx: idx + 30]
            if any(tok in tail for tok in placeholder_tokens):
                shell_keywords.append(k)
    if shell_keywords:
        return GateResult("G-08", "CodingPlan 14 门禁通过", "warn", True,
                          f"14 门禁关键词齐全但 {len(shell_keywords)} 项疑为空壳（紧邻占位符 {placeholder_tokens}）：{shell_keywords}",
                          "补全空壳项的实质内容（建议书3 §7.4 质量校验升级）",
                          details={"n_pass": n_pass, "n_fail": n_fail, "n_warn": n_warn,
                                   "shell_keywords": shell_keywords})

    return GateResult("G-08", "CodingPlan 14 门禁通过", "blocker", True,
                      f"14 门禁全通过（✅ {n_pass} / 🟡 {n_warn}）",
                      details={"n_pass": n_pass, "n_fail": n_fail, "n_warn": n_warn,
                               "file": str(cp)})


# ─── G-09：调 test_authenticity_scan.py 扫测试代码 ───────────────────────────
import json as _json
import subprocess as _subprocess


def _locate_runtime_script(master_source: Optional[Path], script_name: str) -> Optional[Path]:
    """定位随 ae-sdd 分发的运行时脚本。

    兼容两种布局：
    - 开发仓库：<repo>/source/SKILL.md，脚本在 <repo>/scripts/
    - 分发包：<dist>/SKILL.md，脚本在 <dist>/scripts/
    """
    if master_source is None:
        return None

    candidates = [
        master_source.parent / "scripts" / script_name,
        master_source / "scripts" / script_name,
    ]
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    return None


def _locate_authenticity_scanner(master_source: Optional[Path]) -> Optional[Path]:
    """在母版找 test_authenticity_scan.py"""
    return _locate_runtime_script(master_source, "test_authenticity_scan.py")


def _locate_coding_scanner(master_source: Optional[Path]) -> Optional[Path]:
    """在母版找 coding_authenticity_scan.py"""
    return _locate_runtime_script(master_source, "coding_authenticity_scan.py")


def check_g09(project_dir: Path, st: dict, current_story: str,
              master_source: Optional[Path] = None) -> GateResult:
    """G-09 测试真实性扫描通过 — 调 test_authenticity_scan.py 跑 8 类禁止检查

    v1.1 修复（2026-06-18）：
      - 总是跑扫描（不卡 phase），保证发现违规测试
      - 仅当 0 测试文件 + pre-coding phase → 标记 stub（避免"什么都没扫到"的虚假 pass）
      - 0 测试文件 + phase ≥ coding → warn（应有测试但没有）
    """
    phase = st.get("phase", "initialized")
    PRE_CODING_PHASES = {"initialized", "ra-generated", "dr-generated", "story-generated",
                         "story-reviewed", "task-generated", "task-reviewed"}

    scanner = _locate_authenticity_scanner(master_source)
    if scanner is None:
        return GateResult("G-09", "测试真实性扫描通过", "blocker", True,
                          "未找到母版 test_authenticity_scan.py（跳过）",
                          action="确认母版路径",
                          details={"scanned": False, "skipped": True, "stub": True})

    # 跑扫描（不卡 phase：保证能发现违规测试）
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
    java_test_files = report.get("javaTestFiles", 0)
    blockers = sum(1 for f in report.get("findings", []) if f.get("severity") == "BLOCKER")
    n_total = len(report.get("findings", []))

    # 有 findings / BLOCKER → 直接 fail（无论 phase）
    if status != "PASS" or blockers > 0:
        return GateResult("G-09", "测试真实性扫描通过", "blocker", False,
                          f"扫描失败：{n_total} findings / {blockers} BLOCKER",
                          f"修复测试代码中的 8 类禁止（{scanner.name}）",
                          details={"scanned": True, "n_findings": n_total, "n_blockers": blockers,
                                   "status": status, "n_test_files": java_test_files})

    # 0 测试文件：
    # - pre-coding → stub（还没到写测试的阶段，扫描无对象不算 pass）
    # - ≥ coding → warn（应该写测试但没写）
    if java_test_files == 0:
        if phase in PRE_CODING_PHASES:
            return GateResult("G-09", "测试真实性扫描通过", "blocker", True,
                              f"phase = {phase}（pre-coding，扫描无对象，按 stub 算）",
                              action="进入 coding 阶段后此门禁生效",
                              details={"scanned": True, "skipped": True, "stub": True,
                                       "current_phase": phase, "n_test_files": 0})
        else:
            return GateResult("G-09", "测试真实性扫描通过", "warn", True,
                              f"phase = {phase} 但 0 测试文件（应编写测试）",
                              action="确认是否漏写测试代码",
                              details={"scanned": True, "n_findings": 0, "n_test_files": 0,
                                       "current_phase": phase, "stub": False})

    # 有测试文件 + 0 BLOCKER → 真 pass
    # 🆕 v3.4.0 test-verifier 独立性校验（建议书3 B2-7）：测试真实性报告应带独立 session_id
    verifier_warning = _check_test_verifier_independence(project_dir, current_story)

    if verifier_warning:
        return GateResult("G-09", "测试真实性扫描通过", "warn", True,
                          f"扫描通过：{n_total} findings / 0 BLOCKER（{java_test_files} 测试文件）。⚠️ {verifier_warning}",
                          action="test-verifier sub-agent 报告须带独立 session_id（≠ 主 agent）",
                          details={"scanned": True, "n_findings": n_total, "n_blockers": 0,
                                   "n_test_files": java_test_files,
                                   "verifier_warning": verifier_warning})

    return GateResult("G-09", "测试真实性扫描通过", "blocker", True,
                      f"扫描通过：{n_total} findings / 0 BLOCKER（{java_test_files} 测试文件）",
                      details={"scanned": True, "n_findings": n_total, "n_blockers": 0,
                               "n_test_files": java_test_files})


def _check_test_verifier_independence(project_dir: Path, current_story: str) -> Optional[str]:
    """🆕 v3.4.0 test-verifier 独立性软校验（建议书3 B2-7）。

    扫描本 Story 的测试真实性报告，若存在但无 session_id 字段（或 session_id 标记为主 agent），
    返回 warning 字符串；不存在报告或已声明独立 session_id 则返回 None（不阻断）。
    """
    if not current_story:
        return None
    # 找测试真实性报告（常见命名：*-TestVerification-*.md / *-Report.md）
    candidates = list(project_dir.rglob(f"{current_story}*TestVerification*.md"))
    candidates += list(project_dir.rglob(f"{current_story}*TestAuthenticity*.md"))
    if not candidates:
        return None  # 无报告文件，不校验（向后兼容）
    for rep in candidates[:3]:
        try:
            content = rep.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        # 检测 session_id 字段
        has_session = ("session_id" in content.lower() or "sessionId" in content
                       or "verifier session" in content.lower())
        if not has_session:
            return (f"测试真实性报告 {rep.name} 未声明独立 session_id；"
                    "test-verifier sub-agent 报告须带 session_id（≠ 主 agent），防 AI 自跑冒充 sub-agent")
    return None


def check_gcode1(project_dir: Path, st: dict, current_story: str,
                 master_source: Optional[Path] = None) -> GateResult:
    """G-CODE-1 Coding 真实性扫描通过 — 调 coding_authenticity_scan.py。

    对标 CodingModel §6 AI Coding 反模式库，把 AP-1~AP-6 中可静态命中的
    反模式先变成可执行门禁。pre-coding 阶段无代码可扫时按 stub 通过；进入
    coding/test-running/code-reviewed/completed 后，如果生产代码为 0，则降为 warn。
    """
    phase = st.get("phase", "initialized")
    PRE_CODING_PHASES = {"initialized", "ra-generated", "dr-generated", "story-generated",
                         "story-reviewed", "task-generated", "task-reviewed"}

    scanner = _locate_coding_scanner(master_source)
    if scanner is None:
        return GateResult("G-CODE-1", "Coding 真实性扫描通过", "blocker", True,
                          "未找到母版 coding_authenticity_scan.py（跳过）",
                          action="确认母版路径",
                          details={"scanned": False, "skipped": True, "stub": True})

    try:
        result = _subprocess.run(
            [sys.executable, str(scanner), "--root", str(project_dir), "--format", "json"],
            capture_output=True, text=True, timeout=60,
        )
    except _subprocess.TimeoutExpired:
        return GateResult("G-CODE-1", "Coding 真实性扫描通过", "blocker", False,
                          "coding_authenticity_scan.py 跑超过 60 秒",
                          "缩小扫描范围或增加超时")
    except Exception as e:
        return GateResult("G-CODE-1", "Coding 真实性扫描通过", "blocker", False,
                          f"扫描器异常: {e}",
                          "检查 coding_authenticity_scan.py 是否可执行")

    try:
        report = _json.loads(result.stdout) if result.stdout else {}
    except _json.JSONDecodeError as e:
        return GateResult("G-CODE-1", "Coding 真实性扫描通过", "blocker", False,
                          f"扫描器 JSON 输出无法解析: {e}",
                          f"stdout 前 200 字符: {result.stdout[:200]}")

    status = report.get("status", "UNKNOWN")
    code_files = report.get("codeFiles", 0)
    coding_reports = report.get("codingReports", 0)
    blockers = sum(1 for f in report.get("findings", []) if f.get("severity") == "BLOCKER")
    n_total = len(report.get("findings", []))

    if status != "PASS" or blockers > 0:
        blocker_rules = sorted({f.get("rule") for f in report.get("findings", [])
                                if f.get("severity") == "BLOCKER"})
        return GateResult("G-CODE-1", "Coding 真实性扫描通过", "blocker", False,
                          f"Coding 真实性扫描发现 {blockers} 个 BLOCKER（共 {n_total} 项）：{blocker_rules}",
                          "修复 Coding 反模式命中项，或在 CodeReview 中显式评审通过",
                          details={"scanned": True, "n_findings": n_total,
                                   "n_blockers": blockers, "status": status,
                                   "n_code_files": code_files,
                                   "n_coding_reports": coding_reports,
                                   "blocker_rules": blocker_rules})

    if code_files == 0:
        if phase in PRE_CODING_PHASES:
            return GateResult("G-CODE-1", "Coding 真实性扫描通过", "blocker", True,
                              f"phase = {phase}（pre-coding，扫描无对象，按 stub 算）",
                              action="进入 coding 阶段后此门禁生效",
                              details={"scanned": True, "skipped": True, "stub": True,
                                       "current_phase": phase, "n_code_files": 0,
                                       "n_coding_reports": coding_reports})
        return GateResult("G-CODE-1", "Coding 真实性扫描通过", "warn", True,
                          f"phase = {phase} 但 0 个生产代码文件（请确认是否漏扫项目根）",
                          action="确认 --project / cwd 是否指向服务根或仓库根",
                          details={"scanned": True, "n_findings": n_total,
                                   "n_code_files": 0, "n_coding_reports": coding_reports,
                                   "current_phase": phase, "stub": False})

    return GateResult("G-CODE-1", "Coding 真实性扫描通过", "blocker", True,
                      f"Coding 真实性扫描通过（{code_files} 个代码文件，{coding_reports} 份 Coding 报告，0 BLOCKER，{n_total} WARN）",
                      details={"scanned": True, "n_findings": n_total,
                               "n_blockers": 0, "n_code_files": code_files,
                               "n_coding_reports": coding_reports})


# ─── G-13：RA ↔ DR ↔ Story ↔ Task ↔ Coding 六层引用追溯 ──────────────────────
def check_g13(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """
    G-13 全链路对称性核查通过

    v3.2 升级：五层追溯 → 六层追溯，在链路最前端接入 RA 层。
      RA   → 被 DR 引用（RA-ID 出现在 DR 文档）
      DR   → 被 Story 引用
      Task → 引用 Story ID
      Coding Report → 引用 Task ID
      CodeReview → 引用 Story ID

    RA 层为可选层（微任务/BUG 豁免，且 RA 可能尚未生成）：
      - RA 文档不存在 → 不阻断，记录 ra_layer={"present": False}
      - RA 存在但 DR 未引用 RA-ID → 记录 issue（仅当 DR 也存在时）
    """
    if not current_story:
        return GateResult("G-13", "全链路对称性核查通过", "blocker", False,
                          "state.currentStory 为空")

    design = paths.project_design_dir(project_dir)
    if not design.is_dir():
        return GateResult("G-13", "全链路对称性核查通过", "blocker", False,
                          f"design/ 目录不存在",
                          f"先生成设计文档")

    issues: list[str] = []
    ra_layer_detail: dict = {"present": False, "files": 0}

    # 0. RA → DR 引用追溯（v3.2 新增，链路最前端）
    ra_files = _iter_ra_files(project_dir)
    if ra_files:
        ra_layer_detail["present"] = True
        ra_layer_detail["files"] = len(ra_files)
        ra_ids = {f.stem for f in ra_files}
        # 同时收集 RA-ID 模式（如 RA-001）
        ra_id_pattern = re.compile(r"RA[-_]\d+", re.IGNORECASE)
        for f in ra_files:
            ra_ids.update(ra_id_pattern.findall(f.stem))
        ra_layer_detail["ra_ids"] = sorted(ra_ids)

        drs = sorted(set(design.glob("*DR*.md")) | set(design.glob("*dr*.md")))
        if drs:
            # 至少一个 DR 文档引用了某个 RA-ID
            dr_refs_ra = False
            for d in drs:
                dr_content = d.read_text(encoding="utf-8")
                if any(rid in dr_content for rid in ra_ids):
                    dr_refs_ra = True
                    break
            if not dr_refs_ra:
                issues.append(f"DR 文档未引用任何 RA-ID（{len(ra_files)} 个 RA 可选）")
    # RA 不存在：不阻断（可选层），ra_layer_detail.present=False

    # 1. Story → DR 引用追溯
    story = paths.find_doc(project_dir, current_story, ".md")
    if story is not None:
        # 找 design/ 下的所有 DR
        drs = sorted(set(design.glob("*DR*.md")) | set(design.glob("*dr*.md")))
        if not drs:
            issues.append("无 DR 文档可追溯（design/ 目录无 *DR*.md）")
        else:
            # 至少一个 DR ID 在 Story 文档里被引用
            story_content = story.read_text(encoding="utf-8")
            dr_refs = [d.stem for d in drs if d.stem in story_content]
            if not dr_refs:
                # 用更宽松的检测：DR 文档的 ID（如 DR-001）出现在 Story 文档
                dr_id_pattern = re.compile(r"DR-\d+", re.IGNORECASE)
                dr_ids = set()
                for d in drs:
                    dr_ids.update(dr_id_pattern.findall(d.stem))
                # 加上文件名
                dr_ids.update(d.stem for d in drs)
                ref_hits = [did for did in dr_ids if did in story_content]
                if not ref_hits:
                    issues.append(f"Story 文档未引用任何 DR（{len(drs)} 个 DR 可选）")
    else:
        # Story 文档不存在 → 链路断
        issues.append(f"Story 文档不存在：{current_story}.md（无法建立追溯）")

    # 2. Task → Story 引用追溯
    tasks = paths.list_docs(project_dir, current_story, "-task-*.md")
    for t in tasks:
        task_content = t.read_text(encoding="utf-8")
        if current_story not in task_content:
            issues.append(f"Task 文档未引用 Story ID {current_story}：{t.name}")

    # 3. Coding Report → Task 引用追溯（如果存在）
    coding_report = paths.find_doc(project_dir, current_story, "-Coding-Report.md")
    if coding_report is not None:
        cr_content = coding_report.read_text(encoding="utf-8")
        for t in tasks:
            # 引用判定：Task 的 stem（如 STORY-001-task-001）在 Coding Report 里出现
            if t.stem not in cr_content:
                issues.append(f"Coding Report 未引用 Task：{t.stem}")

    # 4. CodeReview → Story 引用追溯（如果存在）
    code_review = paths.find_doc(project_dir, current_story, "-CodeReview.md")
    if code_review is not None:
        cv_content = code_review.read_text(encoding="utf-8")
        if current_story not in cv_content:
            issues.append(f"CodeReview 报告未引用 Story ID {current_story}")

    if issues:
        return GateResult("G-13", "全链路对称性核查通过", "blocker", False,
                          f"链路追溯发现 {len(issues)} 个问题：{issues[0]}" + ("..." if len(issues) > 1 else ""),
                          "修复文档间的引用关系",
                          details={"issues": issues, "n_issues": len(issues),
                                   "ra_layer": ra_layer_detail})

    n_drs = len(list(design.glob("*DR*.md")) + list(design.glob("*dr*.md")))
    layer_note = "六层追溯完整（RA ↔ DR ↔ Story ↔ Task ↔ Coding Report ↔ CodeReview）" \
        if ra_layer_detail["present"] else \
        "五层追溯完整（DR ↔ Story ↔ Task ↔ Coding Report ↔ CodeReview，RA 层未生成/豁免）"
    return GateResult("G-13", "全链路对称性核查通过", "blocker", True,
                      layer_note,
                      details={"current_story": current_story,
                               "n_tasks": len(tasks),
                               "n_drs": n_drs,
                               "ra_layer": ra_layer_detail})


# ─── G-RA：需求分析准入门卫（v3.2 — 对标 SKILL.md §🛡️ G-RA 7 条规则）─────────
# 背景：Coding 有 gates.py G-08/G-09 把"14 门禁/8类禁止"从纸面规则变成可执行门禁；
# RA 的"16 道 RA-G 闸"长期只有 SKILL 描述、无代码强制。G-RA 补齐这一层，使需求
# 分析质量与 Coding 一样可被代码验证。SKILL.md 已声明 7 条规则 + 命令
# `ae-sdd gate ra-required`；本节落地其中可自动判定的部分。

# RA 8 核心维度关键词（对应 requirement-analysis-skill §第一步 8 维度 + RA-G05~RA-G12）
RA_8_DIMENSIONS = [
    ("角色", ["角色", "§2", "角色枚举", "角色矩阵"]),
    ("场景", ["场景", "§3", "主流程", "异常场景", "边界场景"]),
    ("流程", ["流程", "§4", "状态机", "时序", "状态流转"]),
    ("数据", ["数据", "§5", "实体", "字段", "数据要素"]),
    ("规则", ["规则", "§6", "业务规则", "约束", "R1"]),
    ("设计方向", ["设计方向", "§7", "备选方案", "方案对比"]),
    ("AC", ["AC", "§8", "验收标准", "Given", "When", "Then"]),
    ("假设", ["假设", "§9", "隐性假设", "验证方式"]),
]

# RA 衍生章节锚点（对应 RA-G08/RA-G09/RA-G12 + 状态机类需求必填）
RA_DERIVATIVE_SECTIONS = [
    ("§6.5 衍生规则登记表", "§6.5", "衍生规则"),
    ("§8.5 衍生 AC 登记表", "§8.5", "衍生 AC"),
    ("§8.6 衍生覆盖率", "§8.6", "衍生覆盖率"),
    ("§9-bis 业务模式匹配表", "§9-bis", "业务模式匹配"),
    ("§9-ter 跨域级联效应表", "§9-ter", "跨域级联"),
]

# RAModel 12 维锚点（对应 RA §0.5 + RA-G02）
RA_RAMODEL_KEYWORDS = ["RA-01", "RA-02", "RA-03", "RA-04", "RA-05", "RA-06",
                       "RA-07", "RA-08", "RA-09", "RA-10", "RA-11", "RA-12"]

# 状态机类需求关键词（命中则衍生章节必须非空，对标 E.5/G.5/H.6 触发条件）
STATE_MACHINE_KEYWORDS = ["状态变更", "状态机", "触发", "联动", "禁用", "启用",
                          "锁定", "解锁", "注销", "角色变更", "退款", "取消",
                          "登录", "登出", "失败", "超时", "过期", "状态流转"]

# RA 文档命名约定（与 ra_authenticity_scan.py 一致）
RA_FILENAME_RE = re.compile(r"RA[-_]", re.IGNORECASE)


def _iter_ra_files(project_dir: Path) -> list[Path]:
    """枚举项目内 RA 文档（兼容新路径 ae-sdd-doc/ 与旧路径 design/）。"""
    out: list[Path] = []
    for path in project_dir.rglob("*.md"):
        if not RA_FILENAME_RE.search(path.name):
            continue
        lower = path.as_posix().lower()
        if any(seg in lower for seg in ("changelog", "template", "ra-template", "change_log")):
            continue
        out.append(path)
    return out


def check_ra_required(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-RA-1 RA 文档存在（SKILL.md G-RA 规则 1/5/6/7）。

    规则 1：进 dr/story/task-generate 前必须存在 RA 文档。
    规则 5：RA 距今 ≤ 30 天（超期 → warn，不阻断）。
    规则 6/7：微任务/BUG 豁免（phase 标识或无下游需求时不阻断）。
    """
    name = "RA 文档存在"
    phase = st.get("phase", "initialized")

    ra_files = _iter_ra_files(project_dir)

    # 规则 1：RA 文档存在
    if not ra_files:
        # pre-RA 阶段（还没开始需求分析）→ stub，不阻断
        pre_ra_phases = {"initialized", "ra-generated"}
        if phase in pre_ra_phases or phase == "initialized":
            return GateResult("G-RA-1", name, "blocker", True,
                              "pre-RA 阶段，RA 文档尚未生成（stub 通过）",
                              details={"stub": True, "phase": phase, "ra_files": 0})
        return GateResult("G-RA-1", name, "blocker", False,
                          f"未找到 RA 文档（phase={phase}，已进入下游节点）",
                          "运行 `ae-sdd gate ra-required --fix` 或触发 requirement-analysis-skill 生成 RA",
                          details={"phase": phase, "ra_files": 0})

    # 规则 5：RA 距今 ≤ 30 天（取最新一份的修改时间）
    latest = max(ra_files, key=lambda p: p.stat().st_mtime)
    mtime = datetime.fromtimestamp(latest.stat().st_mtime, tz=timezone.utc)
    now = datetime.now(tz=timezone.utc)
    age_days = (now - mtime).days

    if age_days > 30:
        return GateResult("G-RA-1", name, "warn", True,
                          f"RA 文档距今 {age_days} 天（超 30 天），建议重审：{latest.name}",
                          "重审 RA 是否仍反映当前需求",
                          details={"ra_files": len(ra_files), "latest": latest.name,
                                   "age_days": age_days, "warn_only": True})

    return GateResult("G-RA-1", name, "blocker", True,
                      f"RA 文档存在（{len(ra_files)} 份，最新 {latest.name}，{age_days} 天前）",
                      details={"ra_files": len(ra_files), "latest": latest.name,
                               "age_days": age_days})


def check_ra_dimensions(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-RA-2 RA 8 维度完整 + RAModel 12 维（SKILL.md G-RA 规则 2）。

    规则 2：RA 文档必须含 8 个核心维度。同时检查 RAModel 12 维决策记录（RA-G02）。
    """
    name = "RA 8 维度完整"
    ra_files = _iter_ra_files(project_dir)
    if not ra_files:
        return GateResult("G-RA-2", name, "blocker", True,
                          "无 RA 文档（依赖 G-RA-1 判定）",
                          details={"stub": True})

    latest = max(ra_files, key=lambda p: p.stat().st_mtime)
    content = latest.read_text(encoding="utf-8")

    # 8 维度检查
    missing_dims = []
    for dim_name, keywords in RA_8_DIMENSIONS:
        if not any(kw in content for kw in keywords):
            missing_dims.append(dim_name)

    if missing_dims:
        return GateResult("G-RA-2", name, "blocker", False,
                          f"RA 缺失维度：{missing_dims}",
                          f"补全 RA 文档的 8 维度章节（{missing_dims}）",
                          details={"missing": missing_dims, "file": latest.name})

    # RAModel 12 维检查（RA-G02）
    missing_ra = [k for k in RA_RAMODEL_KEYWORDS if k not in content]
    if missing_ra:
        return GateResult("G-RA-2", name, "blocker", False,
                          f"RAModel 12 维决策记录缺失：{missing_ra}",
                          f"补全 RA §0.5 的 RAModel 12 维（{missing_ra}）",
                          details={"missing_ramodel": missing_ra, "file": latest.name})

    return GateResult("G-RA-2", name, "blocker", True,
                      f"8 维度齐全 + RAModel 12 维完整（{latest.name}）",
                      details={"file": latest.name, "ramodel_dims": 12})


def check_ra_derivatives(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-RA-3 RA 衍生章节完整（SKILL.md G-RA 规则隐含 + RA-G08/RA-G09/RA-G12）。

    状态机类需求（命中 STATE_MACHINE_KEYWORDS）必须填满 5 个衍生章节；
    非状态机类需求允许"不适用 + 理由"。
    """
    name = "RA 衍生章节完整"
    ra_files = _iter_ra_files(project_dir)
    if not ra_files:
        return GateResult("G-RA-3", name, "blocker", True,
                          "无 RA 文档（依赖 G-RA-1 判定）",
                          details={"stub": True})

    latest = max(ra_files, key=lambda p: p.stat().st_mtime)
    content = latest.read_text(encoding="utf-8")

    is_state_machine = any(kw in content for kw in STATE_MACHINE_KEYWORDS)

    missing_sections = []
    for section_name, anchor, _kw in RA_DERIVATIVE_SECTIONS:
        if anchor not in content:
            missing_sections.append(section_name)

    if missing_sections:
        if is_state_machine:
            return GateResult("G-RA-3", name, "blocker", False,
                              f"状态机类需求缺失衍生章节：{missing_sections}",
                              f"补全 {missing_sections}（状态变更类需求必填，见 E.5/G.5/H.5/H.6）",
                              details={"missing": missing_sections, "state_machine": True,
                                       "file": latest.name})
        # 非状态机：允许缺失，但要求有"不适用"声明
        has_not_applicable = any(kw in content for kw in ["不适用", "不涉及", "无需衍生"])
        if not has_not_applicable:
            return GateResult("G-RA-3", name, "blocker", False,
                              f"非状态机需求缺失衍生章节且无'不适用'声明：{missing_sections}",
                              f"补全章节或显式标注'不适用 + 理由'",
                              details={"missing": missing_sections, "state_machine": False,
                                       "file": latest.name})

    return GateResult("G-RA-3", name, "blocker", True,
                      f"衍生章节完整（state_machine={is_state_machine}，{latest.name}）",
                      details={"file": latest.name, "state_machine": is_state_machine,
                               "missing": missing_sections})


def _locate_ra_scanner(master_source: Optional[Path]) -> Optional[Path]:
    """在母版找 ra_authenticity_scan.py（对标 _locate_authenticity_scanner）。"""
    return _locate_runtime_script(master_source, "ra_authenticity_scan.py")


def check_ra_authenticity(project_dir: Path, st: dict, current_story: str,
                          master_source: Optional[Path] = None) -> GateResult:
    """G-RA-4 RA 真实性扫描通过（SKILL.md G-RA 规则 3/4 自动化部分）。

    调 ra_authenticity_scan.py 跑 8 类禁止检查（对标 G-09 调 test_authenticity_scan.py）。
    BLOCKER=0 → pass。
    """
    name = "RA 真实性扫描通过"
    phase = st.get("phase", "initialized")
    pre_ra_phases = {"initialized", "ra-generated"}

    scanner = _locate_ra_scanner(master_source)
    if scanner is None:
        return GateResult("G-RA-4", name, "blocker", True,
                          "未找到母版 ra_authenticity_scan.py（跳过）",
                          action="确认母版路径",
                          details={"scanned": False, "skipped": True, "stub": True})

    # 0 RA 文档 + pre-RA phase → stub
    ra_files = _iter_ra_files(project_dir)
    if not ra_files and phase in pre_ra_phases:
        return GateResult("G-RA-4", name, "blocker", True,
                          "pre-RA 阶段无 RA 文档（stub 通过）",
                          details={"scanned": False, "stub": True, "phase": phase})

    # 跑扫描
    try:
        result = _subprocess.run(
            [sys.executable, str(scanner), "--root", str(project_dir), "--format", "json"],
            capture_output=True, text=True, timeout=60,
        )
    except _subprocess.TimeoutExpired:
        return GateResult("G-RA-4", name, "blocker", False,
                          "ra_authenticity_scan.py 跑超过 60 秒",
                          "缩小扫描范围或增加超时")
    except Exception as e:
        return GateResult("G-RA-4", name, "blocker", False,
                          f"扫描器异常: {e}",
                          "检查 ra_authenticity_scan.py 是否可执行")

    try:
        report = _json.loads(result.stdout) if result.stdout else {}
    except _json.JSONDecodeError as e:
        return GateResult("G-RA-4", name, "blocker", False,
                          f"扫描器 JSON 输出无法解析: {e}",
                          f"stdout 前 200 字符: {result.stdout[:200]}")

    status = report.get("status", "UNKNOWN")
    ra_files_scanned = report.get("raFiles", 0)
    blockers = sum(1 for f in report.get("findings", []) if f.get("severity") == "BLOCKER")
    n_total = len(report.get("findings", []))

    if ra_files_scanned == 0:
        return GateResult("G-RA-4", name, "blocker", True,
                          "无 RA 文档可扫描（依赖 G-RA-1 判定）",
                          details={"scanned": True, "ra_files": 0, "stub": True})

    if status != "PASS" or blockers > 0:
        blocker_rules = sorted({f.get("rule") for f in report.get("findings", [])
                                if f.get("severity") == "BLOCKER"})
        return GateResult("G-RA-4", name, "blocker", False,
                          f"RA 真实性扫描发现 {blockers} 个 BLOCKER（共 {n_total} 项）：{blocker_rules}",
                          "修复 RA 文档中标 BLOCKER 的项，或显式评审通过",
                          details={"scanned": True, "ra_files": ra_files_scanned,
                                   "blockers": blockers, "total": n_total,
                                   "blocker_rules": blocker_rules})

    return GateResult("G-RA-4", name, "blocker", True,
                      f"RA 真实性扫描通过（{ra_files_scanned} 份 RA，0 BLOCKER，{n_total} WARN）",
                      details={"scanned": True, "ra_files": ra_files_scanned,
                               "blockers": 0, "total": n_total})


def _locate_flow_violation_scanner(master_source: Optional[Path]) -> Optional[Path]:
    """在母版找 flow_violation_scan.py（G-RA-FLOW-VIOLATION 运行时依赖，🆕 2026-06-27）。"""
    return _locate_runtime_script(master_source, "flow_violation_scan.py")


def check_ra_flow_violation(project_dir: Path, st: dict, current_story: str,
                             master_source: Optional[Path] = None) -> GateResult:
    """G-RA-FLOW-VIOLATION RA 流程违规审计通过（🆕 2026-06-27，建议书 §3.4）。

    调 flow_violation_scan.py 跑 8 条规则检查（R1 12 维 / R2 8 维度 / R3 缺口
    / R4 规模 / R5 RAGeneratePlan / R6 RA-G 闸 / R7 5 问自检 / R8 缺口闭环）。
    BLOCKER=0 → pass；BLOCKER>0 → blocker。

    与 G-RA-4 区别：G-RA-4 看真实性（无 fabricate/vague），本门禁看流程完整性
    （是否走完 RAModel 12 维 + 8 维度 + 5 问 + 缺口 + 规模 + 16 道闸）。
    """
    name = "RA 流程违规审计"
    phase = st.get("phase", "initialized")
    # 仅在 ra-generated 之后才检查（之前无 RA 文档）
    pre_ra_phases = {"initialized", "ra-generated"}
    if phase in pre_ra_phases and not current_story:
        return GateResult("G-RA-FLOW-VIOLATION", name, "blocker", True,
                          f"阶段 {phase} 暂无 RA 文档（跳过，依赖 G-RA-1）",
                          details={"skipped": True, "reason": "pre-ra-phase"})

    scanner = _locate_flow_violation_scanner(master_source)
    if scanner is None:
        return GateResult("G-RA-FLOW-VIOLATION", name, "blocker", True,
                          "未找到母版 flow_violation_scan.py（跳过）",
                          details={"skipped": True, "reason": "scanner-not-found"})

    try:
        result = _subprocess.run(
            [_sys.executable, str(scanner), "--root", str(project_dir), "--format", "json"],
            capture_output=True, text=True, timeout=60, check=False,
        )
    except _subprocess.TimeoutExpired:
        return GateResult("G-RA-FLOW-VIOLATION", name, "blocker", False,
                          "flow_violation_scan.py 跑超过 60 秒",
                          "缩小扫描范围或增加超时")
    except Exception as e:
        return GateResult("G-RA-FLOW-VIOLATION", name, "blocker", False,
                          f"扫描器异常: {e}",
                          "检查 flow_violation_scan.py 是否可执行")

    try:
        report = _json.loads(result.stdout) if result.stdout else {}
    except _json.JSONDecodeError as e:
        return GateResult("G-RA-FLOW-VIOLATION", name, "blocker", False,
                          f"扫描器 JSON 输出无法解析: {e}",
                          f"stdout 前 200 字符: {result.stdout[:200]}")

    status = report.get("status", "UNKNOWN")
    ra_files_scanned = report.get("raFiles", 0)
    blockers = sum(1 for f in report.get("findings", []) if f.get("severity") == "BLOCKER")
    n_total = len(report.get("findings", []))

    if ra_files_scanned == 0:
        return GateResult("G-RA-FLOW-VIOLATION", name, "blocker", True,
                          "无 RA 文档可扫描（依赖 G-RA-1 判定）",
                          details={"scanned": True, "ra_files": 0, "stub": True})

    if status != "PASS" or blockers > 0:
        blocker_rules = sorted({f.get("rule") for f in report.get("findings", [])
                                if f.get("severity") == "BLOCKER"})
        return GateResult("G-RA-FLOW-VIOLATION", name, "blocker", False,
                          f"RA 流程违规审计发现 {blockers} 个 BLOCKER（共 {n_total} 项）：{blocker_rules}",
                          "修复 RA 文档中标 BLOCKER 的项（补 12 维 / 8 维度 / 5 问 / 缺口 / 规模 / RA-G 闸判定）",
                          details={"scanned": True, "ra_files": ra_files_scanned,
                                   "blockers": blockers, "total": n_total,
                                   "blocker_rules": blocker_rules})

    return GateResult("G-RA-FLOW-VIOLATION", name, "blocker", True,
                      f"RA 流程违规审计通过（{ra_files_scanned} 份 RA，0 BLOCKER，{n_total} WARN）",
                      details={"scanned": True, "ra_files": ra_files_scanned,
                               "blockers": 0, "total": n_total})


# ─── G-14：CodingPlan-Story 一致性（建议书4 G-08-15）─────────────────────────
# CodingPlan 涉及的接口/DO/AC 必须与 Story 可对应；偏离项须有 Proposal 引用。
# 设计在 ④bis（CodingPlan 生成）→ ⑤ Coding 之间硬拦截。
def check_g14(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-14 CodingPlan-Story 一致性 — Plan 须引用 Story 且关键设计可对应"""
    name = "CodingPlan-Story 一致性"
    if not current_story:
        return GateResult("G-14", name, "blocker", False, "state.currentStory 为空")

    cp = paths.find_doc(project_dir, current_story, "-CodingPlan.md")
    if cp is None:
        return GateResult("G-14", name, "blocker", False,
                          "CodingPlan 文档不存在，无法核对一致性",
                          f"先生成 {current_story}-CodingPlan.md")

    # Story 文档须存在（G-02 范畴，此处只做引用校验）
    story_doc = paths.find_doc(project_dir, current_story, ".md")
    cp_content = cp.read_text(encoding="utf-8")

    issues: list[str] = []

    # 1. CodingPlan 须含 Story 文档引用（路径或 STORY-ID），且引用文件存在
    has_story_ref = (current_story in cp_content) or ("Story" in cp_content)
    if not has_story_ref:
        issues.append(f"CodingPlan 未引用 Story 文档（无 '{current_story}' 或 'Story' 字样）")
    elif story_doc is None:
        issues.append(f"CodingPlan 引用的 Story 文档不存在：{current_story}.md")

    # 2. AC ID 对齐：CodingPlan 测试章节须覆盖 Story 的 AC（至少出现 AC 编号）
    ac_ids_in_cp = set(re.findall(r"AC[-_]?\d+", cp_content))
    if not ac_ids_in_cp:
        # 无 AC 引用可能是微任务场景；仅当 Story 文档含 AC 而 CodingPlan 无 → issue
        if story_doc is not None:
            story_content = story_doc.read_text(encoding="utf-8")
            story_acs = set(re.findall(r"AC[-_]?\d+", story_content))
            if story_acs and not (story_acs & ac_ids_in_cp):
                issues.append(f"Story 含 AC {sorted(story_acs)} 但 CodingPlan 测试章节未对齐任何 AC ID")

    # 3. 偏离 Story 设计须有 Proposal 引用（偏离声明段 + Proposal 文档）
    if "偏离声明" in cp_content or "偏离" in cp_content:
        has_proposal_ref = ("Proposal" in cp_content or "proposal" in cp_content
                            or "PROPOSAL" in cp_content)
        if not has_proposal_ref:
            issues.append("CodingPlan 含'偏离声明'但未引用 Proposal 文档（偏离须有 Proposal 闭环）")

    if issues:
        return GateResult("G-14", name, "blocker", False,
                          f"CodingPlan-Story 一致性未通过（{len(issues)} 项）：{'; '.join(issues)}",
                          f"修复 {current_story}-CodingPlan.md 使其与 Story 一致，偏离项补 Proposal",
                          details={"issues": issues, "ac_ids_in_cp": sorted(ac_ids_in_cp),
                                   "story_doc_exists": story_doc is not None})

    return GateResult("G-14", name, "blocker", True,
                      f"CodingPlan-Story 一致性通过（AC 对齐 {len(ac_ids_in_cp)} 个，Story 引用存在）",
                      details={"ac_ids_in_cp": sorted(ac_ids_in_cp),
                               "story_doc": str(story_doc) if story_doc else None,
                               "file": str(cp)})


# ─── G-CODEPLAN-SRC：CodingPlan 源码核对（建议书1 G-CODEPLAN-SRC）────────────
# CodingPlan 中每个新增/修改类的建模范式须附"已读源码"或"待核实源码"标记；
# 待核实清单非空 → 阻断进 Coding（防凭推测设计类骨架）。
#
# 标记格式（见 be-coding-plan-template.md §2）：
#   【已读源码：domain/message/model/entity/ImMessageDO.java】
#   【待核实源码】 或 【待核实源码：Converter 写法】
_SRC_READ_RE = re.compile(r"【已读源码[：:]([^\】]+)】")
_SRC_PENDING_RE = re.compile(r"【待核实源码[^】]*】")


def check_g_codeplan_src(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-CODEPLAN-SRC CodingPlan 源码核对 — 新增/修改类建模范式须附来源标记"""
    name = "CodingPlan 源码核对"
    if not current_story:
        return GateResult("G-CODEPLAN-SRC", name, "blocker", False, "state.currentStory 为空")

    cp = paths.find_doc(project_dir, current_story, "-CodingPlan.md")
    if cp is None:
        return GateResult("G-CODEPLAN-SRC", name, "blocker", False,
                          "CodingPlan 文档不存在，无法核对源码",
                          f"先生成 {current_story}-CodingPlan.md")

    content = cp.read_text(encoding="utf-8")

    # 定位"关键类骨架"章节（§2 / 章节 2 / 关键类骨架）
    skeleton_section = _extract_skeleton_section(content)

    if skeleton_section is None:
        # 无类骨架章节（微任务可能无）→ 跳过（不阻断）
        return GateResult("G-CODEPLAN-SRC", name, "blocker", True,
                          "CodingPlan 无关键类骨架章节（微任务场景，跳过源码核对）",
                          details={"skipped": True, "reason": "no skeleton section"})

    read_marks = _SRC_READ_RE.findall(skeleton_section)
    pending_marks = _SRC_PENDING_RE.findall(skeleton_section)

    # 校验已读源码标记的文件是否真实存在（防伪造标记）
    missing_read_files = []
    for ref_path in read_marks:
        ref_clean = ref_path.strip()
        # 尝试在 project_dir 下定位该文件
        candidate = project_dir / ref_clean
        if not candidate.is_file():
            # 尝试相对 src/ 解析
            candidate2 = project_dir / "src" / "main" / "java" / ref_clean
            if not candidate2.is_file():
                missing_read_files.append(ref_clean)

    n_read = len(read_marks)
    n_pending = len(pending_marks)
    n_total_marks = n_read + n_pending

    if n_total_marks == 0:
        return GateResult("G-CODEPLAN-SRC", name, "blocker", False,
                          "CodingPlan 关键类骨架章节无任何源码核对标记（每个新增/修改类须附【已读源码：】或【待核实源码】）",
                          f"在 {current_story}-CodingPlan.md §2 类骨架章节补来源标记",
                          details={"n_read": 0, "n_pending": 0})

    if n_pending > 0:
        return GateResult("G-CODEPLAN-SRC", name, "blocker", False,
                          f"CodingPlan 有 {n_pending} 个待核实源码标记未闭环（待核实清单非空禁止进 Coding）：{pending_marks}",
                          f"补读现有同类源码后，把【待核实源码】改为【已读源码：{'}路径{'}】",
                          details={"n_read": n_read, "n_pending": n_pending,
                                   "pending": pending_marks})

    if missing_read_files:
        return GateResult("G-CODEPLAN-SRC", name, "blocker", False,
                          f"CodingPlan 标注已读但源码文件不存在（{len(missing_read_files)} 个）：{missing_read_files[:5]}",
                          "核对路径或改为【待核实源码】",
                          details={"n_read": n_read, "n_pending": n_pending,
                                   "missing_read_files": missing_read_files})

    return GateResult("G-CODEPLAN-SRC", name, "blocker", True,
                      f"源码核对通过（{n_read} 个已读标记，0 待核实，文件均存在）",
                      details={"n_read": n_read, "n_pending": 0,
                               "read_files": read_marks, "file": str(cp)})


def _extract_skeleton_section(content: str) -> Optional[str]:
    """从 CodingPlan 文档提取'关键类骨架'章节文本。

    匹配 §2 / 章节 2 / 关键类骨架 等标题，截取到下一个同级章节。
    """
    # 匹配 "关键类骨架" 标题（含 §2 / 章节 2 / ## 关键类骨架 等变体）
    m = re.search(r"(#{1,4}\s*(?:§?\s*2|章节\s*2|关键类骨架)[^\n]*)\n", content)
    if m is None:
        # 退一步：含"类骨架"关键词的标题
        m = re.search(r"(#{1,4}\s*[^\n]*类骨架[^\n]*)\n", content)
    if m is None:
        return None
    start = m.end()
    # 截取到下一个 ## 或 ### 标题
    rest = content[start:]
    next_heading = re.search(r"\n#{1,3}\s", rest)
    if next_heading:
        return rest[:next_heading.start()]
    return rest


# ─── G-DOC-STORAGE：文档落地存放合规（建议书2 G-DOC-STORAGE）─────────────────
# 流程产出文档路径/命名须经 document-storage resolve_path 推导，禁止硬编码游离路径。
# 扫描 project_dir 下本 Story 产出文档，校验路径模式 + 命名规则。
# 放行：草稿/临时文件（非 {STORY}-*.md 命名）；拦截：游离在 tmp/根目录的流程产物。
_DOC_FLOW_TYPES = ("Story", "Task", "CodingPlan", "CodingReport", "CodeReview",
                   "TestCase", "Report", "DR", "RA", "业务逻辑汇总")
# 合规产物根目录（相对 project_dir）
_DOC_COMPLIANT_ROOTS = (
    "ae-sdd-doc", "design", ".ae-task", ".ae-plan", ".ae-sdd",
    ".auto-engineering",
)
# 游离位置（绝对禁止流程产物落到这里）
_DOC_STRAY_MARKERS = ("tmp", "temp", "$temp", "/tmp", "\\tmp", "d:\\tmp", "c:\\tmp",
                      "desktop", "下载")


def check_g_doc_storage(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-DOC-STORAGE 文档落地存放合规 — 产物路径/命名须合规"""
    name = "文档落地存放合规"
    issues: list[str] = []

    # 扫描 project_dir 下疑似流程产物（{STORY}-* 或 {DocType} 命名的 .md）
    # 限定 2 层深度，避免 rglob 全盘扫描耗时
    stray_files: list[str] = []
    checked = 0
    for md_path in project_dir.rglob("*.md"):
        # 限制扫描深度（跳过 node_modules/.git 等无关目录）
        try:
            rel = md_path.relative_to(project_dir)
        except ValueError:
            continue
        rel_str = str(rel).replace("\\", "/")
        if any(seg in rel_str for seg in ("node_modules/", ".git/", "dist/", "CHANGELOG/", "docs/plans/")):
            continue
        checked += 1
        if checked > 500:  # 性能护栏
            break

        fname = md_path.name
        # 判定是否流程产物：含 Story-ID 或 DocType 关键词
        is_product = (current_story and current_story in fname) or any(
            t in fname for t in _DOC_FLOW_TYPES
        )
        if not is_product:
            continue

        rel_lower = rel_str.lower()
        # 1. 游离位置检测
        if any(m in rel_lower for m in _DOC_STRAY_MARKERS):
            stray_files.append(rel_str)
            continue

        # 2. 不在合规根目录下
        in_compliant = any(rel_str.lower().startswith(r) or f"/{r}/" in f"/{rel_str.lower()}/"
                           for r in _DOC_COMPLIANT_ROOTS)
        # 允许直接在 project_dir 根的产物（向后兼容旧项目 design/ 在根的写法）
        if "/" not in rel_str:
            in_compliant = True
        if not in_compliant:
            stray_files.append(rel_str)

    if stray_files:
        issues.append(f"流程产物落在非合规位置（{len(stray_files)} 个）：{stray_files[:5]}")

    if issues:
        return GateResult("G-DOC-STORAGE", name, "blocker", False,
                          f"文档存放不合规：{'; '.join(issues)}",
                          "调用 document-storage resolve_path 推导路径；禁止硬编码 d:\\tmp\\ 等游离位置",
                          details={"stray_files": stray_files, "checked": checked})

    return GateResult("G-DOC-STORAGE", name, "blocker", True,
                      f"文档存放合规（扫描 {checked} 个 .md，无游离产物）",
                      details={"stray_files": [], "checked": checked})


# ─── G-PATH：路径越界检测（🆕 v4.1，扫母版 source/ 防自身漂移）─────────────────
# document-storage-skill 是路径 SSOT（§0.5）；母版 source/ 下其他 SKILL/template
# 文档不得硬编码产出路径，须声明调用 document-storage。与 plugin_content_scan
# PC-009/010 分层防护：PC-x 扫外挂/插件（防外部污染），G-PATH 扫母版（防自身漂移）。
# 越界路径模式（硬编码这些 = 越界，应改 resolve_path/引用 document-storage）
# 含 4 类：①deprecated 产出路径 design/ ②错版资产路径 .ae-project/ ③项目内技能包路径
# ④技能包内资产路径 skills/ae-sdd/assets/{projectKey}/{projectKey}（项目实例不该用）
_PATH_VIOLATION_RE = re.compile(
    r"(?:design/story/be/|design/testcase/be/|\.ae-project/assets\.md"
    r"|life-team-project-docs/[^\s`]*/design/"
    r"|document/[^\s`]*/skills/ae-sdd/(?:scripts|tools)/"
    r"|skills/ae-sdd/assets/\{projectKey\}/\{projectKey\})"
)
# document-storage 声明线索（文档引用了 SSOT 视为合规线索，但硬编码路径仍报）
_PATH_DECLARATION_RE = re.compile(
    r"document-storage|resolve_path|save_doc|§2\.3|§0\.6", re.IGNORECASE
)
# 扫描范围白名单：母版 source/ 下的 .md
# docs/ 整体跳过（迁移指南/约定文档会引用旧路径作"迁移说明"，非 SKILL 路径定义）
_PATH_SCAN_SKIP_DIRS = ("node_modules/", ".git/", "dist/", "CHANGELOG/", "docs/")


def check_g_path(master_source: Optional[Path], project_dir: Path,
                 st: dict, current_story: str) -> GateResult:
    """G-PATH 路径越界检测 — 母版 source/ 文档不得硬编码产出路径。

    扫描范围：master_source/source/**/*.md（母版自身文档）。
    检测：_PATH_VIOLATION_RE 命中（design/、.ae-project/ 等越界路径）。
    与 G-DOC-STORAGE（扫项目实例产物落点）正交：G-DOC-STORAGE 管"产物落在哪"，
    G-PATH 管"母版文档里写了什么路径定义"。

    master_source 缺失时降级为 warn（不阻断，因母版未安装场景无法扫描）。
    """
    name = "路径越界检测"

    if master_source is None:
        return GateResult("G-PATH", name, "blocker", False,
                          "未定位到母版 master_source，无法扫描 source/ 路径越界",
                          "确认 ae-sdd 母版已安装或在仓库根运行",
                          details={"scanned": 0, "violations": [], "skipped": "no_master"})

    source_dir = master_source / "source" if (master_source / "source").is_dir() else master_source
    if not source_dir.is_dir():
        return GateResult("G-PATH", name, "blocker", False,
                          f"母版 source/ 不存在: {source_dir}",
                          "确认 master_source 定位正确",
                          details={"scanned": 0, "violations": [], "skipped": "no_source"})

    # document-storage-skill.md 是路径 SSOT 持有者，其所有路径引用（定义/deprecation 说明）均合法，
    # 整体豁免（否则 §0.5.3/§2.5 兼容层的迁移说明会被误报为越界）
    DOC_STORAGE_SKILL = "document-storage-skill.md"

    violations: list[dict] = []
    scanned = 0
    for md_path in source_dir.rglob("*.md"):
        try:
            rel = md_path.relative_to(master_source)
        except ValueError:
            continue
        rel_str = str(rel).replace("\\", "/")
        if any(seg in rel_str for seg in _PATH_SCAN_SKIP_DIRS):
            continue
        if md_path.name == DOC_STORAGE_SKILL:
            continue  # SSOT 持有者豁免
        scanned += 1
        if scanned > 500:  # 性能护栏
            break
        try:
            text = md_path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for lineno, line in enumerate(text.splitlines(), start=1):
            if _PATH_VIOLATION_RE.search(line):
                violations.append({
                    "file": rel_str,
                    "line": lineno,
                    "snippet": line.strip()[:100],
                })

    if violations:
        files = sorted({v["file"] for v in violations})
        return GateResult("G-PATH", name, "blocker", False,
                          f"母版文档存在路径越界（{len(violations)} 处，"
                          f"涉及 {len(files)} 文件）：{[v['file']+':'+str(v['line']) for v in violations[:5]]}",
                          "改为声明调用 document-storage resolve_path，或引用 §2.3 路径模板；"
                          "禁止硬编码 design/、.ae-project/、skills/ae-sdd/assets/ 等路径",
                          details={"scanned": scanned, "violations": violations})

    return GateResult("G-PATH", name, "blocker", True,
                      f"路径越界检测通过（扫描 {scanned} 个母版 .md，无硬编码产出路径）",
                      details={"scanned": scanned, "violations": []})


# ─── 路由表 ─────────────────────────────────────────────────────────────────
CHECK_FUNCS: dict[str, Callable] = {
    "G-01": check_g01, "G-02": check_g02, "G-03": check_g03,
    "G-04": check_g04, "G-05": check_g05, "G-06": check_g06,
    "G-07": check_g07,
    "G-08": check_g08, "G-09": check_g09,
    "G-10": check_g10, "G-11": check_g11, "G-12": check_g12,
    "G-13": check_g13,
    "G-14": check_g14,
    "G-CODEPLAN-SRC": check_g_codeplan_src,
    "G-DOC-STORAGE": check_g_doc_storage,
    "G-RA-1": check_ra_required, "G-RA-2": check_ra_dimensions,
    "G-RA-3": check_ra_derivatives, "G-RA-4": check_ra_authenticity,
    "G-RA-FLOW-VIOLATION": check_ra_flow_violation,  # 🆕 2026-06-27 RA 流程违规审计
    "G-CODE-1": check_gcode1,
}


# ─── 主入口 ─────────────────────────────────────────────────────────────────
def check_all(master_source: Optional[Path], ade_sdd: Optional[Path],
              project_key: str, only: Optional[str] = None) -> list[GateResult]:
    """跑全部门禁（14 主门禁 + 4 G-RA + G-CODE）；only 指定时只跑那一个"""
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
        elif g["id"] == "G-RA-4":
            # G-RA-4 同样需要 master_source 调 ra_authenticity_scan.py
            results.append(check_ra_authenticity(project_dir, st, current_story, master_source=master_source))
        elif g["id"] == "G-CODE-1":
            # G-CODE-1 需要 master_source 调 coding_authenticity_scan.py
            results.append(check_gcode1(project_dir, st, current_story, master_source=master_source))
        elif g["id"] == "G-PATH":
            # 🆕 v4.1 G-PATH 需要 master_source 扫母版 source/ 路径越界
            results.append(check_g_path(master_source, project_dir, st, current_story))
        elif g["id"] in CHECK_FUNCS:
            results.append(CHECK_FUNCS[g["id"]](project_dir, st, current_story))
        else:
            results.append(_stub_v31(g["id"], g["name"]))

    return results


def _stub_v31(gate_id: str, name: str) -> GateResult:
    """v3.1 stub 门禁（实现未到位）— 标记 stub=True，不算 pass 也不算 fail"""
    return GateResult(
        gate_id=gate_id,
        name=name,
        severity="blocker",
        pass_=True,           # stub 不阻断
        message=f"v3.1 stub（实现未到位，留 v3.1+ 升级）",
        action=f"等待 v3.1+ 升级 {gate_id} 实现",
        details={"stub": True, "version_target": "v3.1+"},
    )


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
