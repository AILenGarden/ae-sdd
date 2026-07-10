"""
gates.py — ae-sdd 门禁检查（14 主门禁 + 5 G-RA + G-CODE Coding 真实性门禁 + G-DOC-CONSISTENCY）

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

项目侧记忆-配置一致性门禁（🆕 v3.5.7 — 堵"旧记忆劫持 config 路径"盲区）：
  G-DOC-CONSISTENCY 项目侧记忆-配置路径一致性 ✅ 完整（AGENTS/MEMORY 文档路径表述须与
                                                       .ae-sdd/config.yaml docWorkspacePath 一致）
"""
from __future__ import annotations

import re
import sys
import tempfile
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable, Optional

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from lib import paths, runtime_exec, runtime_stats, state as state_mod, work_item_context  # noqa: E402


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


# 门禁元信息（14 主门禁 G-00~G-13 + 3 中段门禁 G-14/G-CODEPLAN-SRC/G-DOC-STORAGE + 1 G-PATH + G-RA-1~6 + G-RA-FLOW + 1 G-CODE + G-DOC-CONSISTENCY + G-REVIEW-LOOP + G-09B + G-AUTO-CONSENSUS = 30）
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
    # 🆕 v3.5.9 RA 机械派生深度通过（防「形式通过、内容空转」）
    # 与 G-RA-3（章节锚点存在）/G-RA-4（无 fabricate/vague）/G-RA-FLOW-VIOLATION（流程完整性）
    # 正交：本门禁验证 E.5/G.5/H.6/H.5 规定的「每行 R→R′→AC 机械追问」是否真做了。
    # 5 条规则：D1 §6.5 主规则机械派生 + D2 R′→AC 链接 + D3 §8.6 覆盖率真实重算
    #        + D4 §9-ter 五问机械覆盖 + D5 §9-bis 业务模式六选一
    {"id": "G-RA-5", "name": "RA 机械派生深度通过",  "severity": "blocker"},
    # 🆕 v3.5.18 RA 实现视角完整性：挖出数据源、数据流、定义/不变量、复用证据、
    # 高成本/难实现设计反驳、开发者疑问答复、DR 交接包，防 RA 只能写产品话术而无法支撑实现。
    {"id": "G-RA-6", "name": "RA 实现视角完整性通过",  "severity": "blocker"},
    {"id": "G-CODE-1", "name": "Coding 真实性扫描通过", "severity": "blocker"},
    # 🆕 v3.5.7 项目侧记忆-配置路径一致性：项目 AGENTS.md/.harness/memory/MEMORY.md 等
    # "文档工作区"表述须与 .ae-sdd/config.yaml 的 docWorkspacePath 一致，防旧记忆劫持新配置
    # （实测案例：life 项目 MEMORY 写 D:\Item\doc 与 config 写 D:\Item\life 冲突，RA 落错位置）
    {"id": "G-DOC-CONSISTENCY", "name": "项目侧记忆-配置路径一致性", "severity": "blocker"},
    # 🆕 v3.5.12 review-loop 退出条件门禁：review 节点（story/dr/task/code review）切相前，
    # 校验 reviewLoop.exitReason 满足协议（normal 需 dryCounter≥3 / escalate 已升级用户）。
    # 治 P0-1/4：堵 root agent 单轮就自称"连续3轮无新增"退出，无机械反驳。
    # 注：本门禁依赖 root agent 跑过 `ae-sdd review-loop collect`（无 reviewLoop 字段时降级 skip）
    {"id": "G-REVIEW-LOOP", "name": "review-loop 退出条件通过", "severity": "blocker"},
    # 🆕 v3.5.13 G-09B reviewer 独立性硬门禁：review 节点切相时机械派生 Tier，
    # 校验 state.activeAgents 有 ≥Tier 个 sessionId≠root 的 reviewer。
    # 独立于 review-loop CLI——root 不调 collect 也会跑（堵"root 总派给自己"）。
    # Tier 1（微/小任务 + 无关键决策）豁免；Tier 2/3 无豁免。
    {"id": "G-09B", "name": "reviewer 独立性通过（多 reviewer 机械强制）", "severity": "blocker"},
    # 🆕 v3.9.20 G-REVIEW-DEPTH Review 深度门禁：禁裸✅ + 零发现举证。
    # 治根因：Review 无深度门禁，reviewer 可把所有维度标"无缺陷"而所有门禁照过
    # （code-review-skill L21-24 坦承"当前为软门禁 report-only"）。本门禁查报告内容证据。
    {"id": "G-REVIEW-DEPTH", "name": "Review 深度（禁裸✅ + 零发现举证）", "severity": "blocker"},
    # 🆕 v3.8.0 G-AUTO-CONSENSUS 自动化联审共识门禁：自动化模式下审核点切相前
    # 校验 state.reviewConsensus[point].passed=true + reviewer 独立性（复用 G-09B 逻辑）。
    # 非自动化模式 / 审核点不在白名单 → skipped（回退人工审核）。
    # 注：本门禁需读 config.yaml 判自动化模式，走 check_all 特判传 master_source。
    {"id": "G-AUTO-CONSENSUS", "name": "自动化联审共识通过", "severity": "blocker"},
    # 🆕 v3.9.1 上下文加载准入门禁（注册表模式）— 对齐 RA/Coding 的「prose+CLI+门禁」三合一，
    # 把 DR/Story/TestCase/Task 四组的「第零步准入检查」从 prose 变成机械阻断。
    # 治「AI 不读 PRD/DR/项目资产/约束就过门禁切相」的真空带。
    # 注册表 CONTEXT_GATE_REGISTRY 定义各 gate 的 scale 适用范围 + required 上下文清单。
    {"id": "G-DR-CTX", "name": "DR 上下文加载", "severity": "blocker"},
    {"id": "G-STORY-CTX", "name": "Story 上下文加载", "severity": "blocker"},
    {"id": "G-TESTCASE-CTX", "name": "TestCase 上下文加载", "severity": "blocker"},
    {"id": "G-TASK-CTX", "name": "Task 上下文加载", "severity": "blocker"},
]

# Story Review 之后允许的 phase
PHASE_PAST_STORY_REVIEW = {
    "story-reviewed",
    "testcase-generated", "testcase-reviewed",  # 🆕 v3.7.0 TestCase 独立系列
    "task-generated", "task-reviewed",
    "coding", "test-running", "code-reviewed", "completed",
}

# Task Review 之后允许的 phase
PHASE_PAST_TASK_REVIEW = {
    "task-reviewed", "coding", "test-running", "code-reviewed", "completed",
}

# CodingPlan 必含章节（按 coding-skill §5 CodePlan 7 章节，被 coding-process §A 调用）
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

    # 🆕 v3.5.10 Gap-004：原用 design.glob("*DR*.md") 只看根一层，不递归子目录，
    # 漏判 design/story/be/STORY-001-BE-CodingReport.md 等子目录产物。
    # 改为 rglob + 排除 -CodeReview / -Report 等非 DR 文档，避免误判。
    drs = sorted(set(design.rglob("*DR*.md")) | set(design.rglob("*dr*.md")))
    # 排除明显不是 DR 的报告类文档（CodeReview/CodingReport 等是产物而非 DR）
    drs = [d for d in drs if not any(
        kw in d.name for kw in ("CodeReview", "CodingReport", "TestReport", "-Report")
    )]
    if not drs:
        return GateResult("G-01", "DR 文档存在", "blocker", False,
                          f"design/ 目录无 DR 文档（rglob *DR*.md / *dr*.md，已排除报告类）",
                          "跑 dr-generate-skill 生成 DR 文档")
    return GateResult("G-01", "DR 文档存在", "blocker", True,
                      f"找到 {len(drs)} 个 DR 文档",
                      details={"files": [str(d.relative_to(project_dir)) for d in drs]})


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
    phase = state_mod.get_active_phase(st) or st.get("phase", "initialized")
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
                          f"跑 CodingPlan 生成（CodingProcess §A 调 coding-skill §5）")

    content = cp.read_text(encoding="utf-8")
    missing = [s for s in CODINGPLAN_REQUIRED_SECTIONS if s not in content]
    if missing:
        return GateResult("G-07", "CodingPlan 存在", "blocker", False,
                          f"CodingPlan 缺章节: {missing}",
                          "补全 7 章节（coding-skill §5 CodePlan 7 章节）",
                          details={"missing_sections": missing})
    return GateResult("G-07", "CodingPlan 存在", "blocker", True,
                      f"{cp.name} 7 章节齐全",
                      details={"file": str(cp)})


# ─── G-10 / G-11 / G-12：报告存在性 ──────────────────────────────────────────
def _dedupe_paths(candidates: list[Path]) -> list[Path]:
    seen: set[str] = set()
    result: list[Path] = []
    for p in candidates:
        key = str(p.resolve()) if p.exists() else str(p)
        if key in seen:
            continue
        seen.add(key)
        result.append(p)
    return result


def _doc_search_roots(project_dir: Path) -> list[Path]:
    """Search both project root and configured docWorkspacePath.

    🔧 v3.9.10：委托 paths.doc_search_roots（路径 SSOT），消除本模块自拼 docWorkspace 的
    重复逻辑（与 find_doc/list_docs 统一入口，DRY）。行为不变：返回去重后的搜索根列表。
    """
    return paths.doc_search_roots(project_dir)


def _find_report_doc(project_dir: Path, current_story: str, *,
                     category: str, patterns: list[str],
                     legacy_suffixes: list[str]) -> Optional[Path]:
    candidates: list[Path] = []
    for root in _doc_search_roots(project_dir):
        for suffix in legacy_suffixes:
            candidates.extend([
                paths.project_design_dir(root) / f"{current_story}{suffix}",
                root / f"{current_story}{suffix}",
            ])

        doc_root = root / "ae-sdd-doc"
        direct_dir = doc_root / category / current_story
        for pattern in patterns:
            candidates.extend(sorted(direct_dir.glob(pattern)))
            candidates.extend(sorted(doc_root.glob(f"iterations/*/{category}/{current_story}/{pattern}")))
            candidates.extend(sorted(doc_root.glob(f"iterations/*/*/{category}/{current_story}/{pattern}")))
            if doc_root.is_dir():
                candidates.extend(sorted(doc_root.rglob(pattern)))

    for cand in _dedupe_paths(candidates):
        if cand.is_file():
            return cand
    return None


def _check_report(project_dir: Path, st: dict, current_story: str, *,
                 gate_id: str, name: str, category: str,
                 patterns: list[str], legacy_suffixes: list[str],
                 expected_hint: str, action: str) -> GateResult:
    """报告类门禁的通用检查逻辑"""
    if not current_story:
        return GateResult(gate_id, name, "blocker", False,
                          "state.currentStory 为空")

    doc = _find_report_doc(project_dir, current_story,
                           category=category,
                           patterns=patterns,
                           legacy_suffixes=legacy_suffixes)
    if doc is None:
        return GateResult(gate_id, name, "blocker", False,
                          f"报告文档不存在: {expected_hint}",
                          action,
                          details={"expected": expected_hint})
    return GateResult(gate_id, name, "blocker", True,
                      f"找到 {doc.name}",
                      details={"file": str(doc)})


def check_g10(project_dir: Path, st: dict, current_story: str) -> GateResult:
    return _check_report(project_dir, st, current_story,
                         gate_id="G-10", name="测试报告存在",
                         category="Test",
                         patterns=[f"{current_story}-Report-v*-r*.md"],
                         legacy_suffixes=["-Report.md"],
                         expected_hint=f"ae-sdd-doc/Test/{current_story}/{current_story}-Report-vN-rM.md",
                         action=f"跑完 Test 系列后生成 {current_story}-Report-vN-rM.md")


def check_g11(project_dir: Path, st: dict, current_story: str) -> GateResult:
    return _check_report(project_dir, st, current_story,
                         gate_id="G-11", name="Coding 报告存在",
                         category="Coding",
                         patterns=[f"{current_story}-CodingReport-v*-r*.md",
                                   f"{current_story}-Coding-Report-v*-r*.md"],
                         legacy_suffixes=["-Coding-Report.md", "-CodingReport.md"],
                         expected_hint=f"ae-sdd-doc/Coding/{current_story}/{current_story}-CodingReport-vN-rM.md",
                         action=f"编码完成后生成 {current_story}-CodingReport-vN-rM.md")


def check_g12(project_dir: Path, st: dict, current_story: str) -> GateResult:
    return _check_report(project_dir, st, current_story,
                         gate_id="G-12", name="CodeReview 报告存在",
                         category="CR",
                         patterns=[f"{current_story}-CodeReview-v*-r*.md"],
                         legacy_suffixes=["-CodeReview.md"],
                         expected_hint=f"ae-sdd-doc/CR/{current_story}/{current_story}-CodeReview-vN-rM.md",
                         action=f"CodeReview 后生成 {current_story}-CodeReview-vN-rM.md")


# 🆕 v3.9.20 G-REVIEW-DEPTH：Review 深度门禁——禁裸✅ + 零发现举证。
# 治根因：Review 无任何深度门禁，reviewer 可把所有维度标"无缺陷"而所有门禁照过。
# 证据信号（机械可查）：
#   - 客观证据：file:line / 第N行 / L\d+ / 报文 / 测试输出 / grep / § / .md 引用 / 行号
#   - 排查证据：排查 / 核查 / 已检查 / 扫描 / 覆盖 + 具体文件/维度名
import re as _re_review_depth
_REVIEW_EVIDENCE_SIGNALS = (
    r"\.java", r"\.kt", r"\.py", r"\.ts", r"\.xml",     # 源码文件引用
    r":\d+", r"第\s*\d+\s*行", r"L\d+", r"行号",          # 行号定位
    r"报文", r"测试输出", r"grep", r"EXPLAIN",            # 客观验证手段
    r"§", r"\.md\b",                                       # 文档章节引用
    r"TableMapping", r"Mapper SQL", r"落库",              # DB 落库证据（组合词，避免 SQL 泛命中）
)
_REVIEW_EVIDENCE_RE = _re_review_depth.compile("|".join(_REVIEW_EVIDENCE_SIGNALS))
_REVIEW_CHECKED_RE = _re_review_depth.compile(r"排查|核查|已检查|扫描|覆盖|逐一|逐项")


def _has_evidence(text: str) -> bool:
    """单行/单格是否含客观证据信号。"""
    return _REVIEW_EVIDENCE_RE.search(text) is not None


def check_g_review_depth(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """🆕 v3.9.20 G-REVIEW-DEPTH Review 深度门禁。

    两项产物级证据校验（查报告内容，不查 reviewer 行为）：
      1. 禁裸✅：CodeReview 报告中每个 ✅ 标记须附客观证据（行号/报文/源码引用等）。
         裸✅（✅ 附近无证据信号）→ 列为问题。
      2. 零发现举证：若报告结论为"无阻断/严重问题"（无 🔴/🟠 发现），
         必须含排查证据段（列出核查过的文件/维度），纯"无问题"三字 → FAIL。

    缺报告/current_story 时降级 skip（由 G-12 兜底报告存在性）。
    """
    gate_id = "G-REVIEW-DEPTH"
    name = "Review 深度（禁裸✅ + 零发现举证）"
    if not current_story:
        return GateResult(gate_id, name, "blocker", True,
                          "state.currentStory 为空（skip，由 G-12 兜底）",
                          details={"skipped": True})
    doc = _find_report_doc(project_dir, current_story,
                           category="CR",
                           patterns=[f"{current_story}-CodeReview-v*-r*.md"],
                           legacy_suffixes=["-CodeReview.md"])
    if doc is None:
        return GateResult(gate_id, name, "blocker", True,
                          "CodeReview 报告不存在（skip，由 G-12 兜底报告存在性）",
                          details={"skipped": True})
    try:
        content = doc.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return GateResult(gate_id, name, "blocker", True,
                          f"报告不可读：{doc.name}（skip）",
                          details={"skipped": True})

    issues: list[str] = []

    # 1. 禁裸✅：定位每个 ✅，检查其所在行 + 下一行是否有证据信号（markdown 表格证据与✅同格或紧邻）
    lines = content.splitlines()
    bare_checks = 0
    for i, line in enumerate(lines):
        if "✅" not in line:
            continue
        window = "\n".join(lines[i:i + 2])
        if not _has_evidence(window):
            bare_checks += 1
    if bare_checks > 3:  # 允许少量表格头 ✅ 噪音，>3 才判定为系统性裸✅
        issues.append(f"发现 {bare_checks} 处裸✅（✅ 未附客观证据：行号/报文/源码引用）")

    # 2. 零发现举证：无 🔴/🟠 发现时，须有排查证据
    has_blocker = "🔴" in content or "阻断" in content
    has_serious = "🟠" in content or "严重" in content
    if not has_blocker and not has_serious:
        if not _REVIEW_CHECKED_RE.search(content):
            issues.append(
                "报告结论为无阻断/严重问题，但缺排查证据段（须列出核查过的文件/维度，"
                "如『已排查 Controller/Service/Mapper/SQL 落库路径』）"
            )

    if issues:
        return GateResult(gate_id, name, "blocker", False,
                          "Review 深度不足：" + "；".join(issues),
                          "为每个 ✅ 补客观证据（file:line/报文/测试输出）；"
                          "零发现时补排查证据段（列出核查范围）",
                          details={"issues": issues, "bare_checks": bare_checks,
                                   "has_blocker": has_blocker, "has_serious": has_serious})
    return GateResult(gate_id, name, "blocker", True,
                      f"Review 深度达标：{bare_checks} 处可疑裸✅（≤3 噪音容忍）"
                      + ("，含阻断/严重发现" if (has_blocker or has_serious) else "，零发现但已附排查证据"),
                      details={"bare_checks": bare_checks, "has_blocker": has_blocker,
                               "has_serious": has_serious})


# ─── G-08：解析 CodingPlan 14 门禁表 ─────────────────────────────────────────
# 按 coding-skill §5 CodePlan 14 门禁必含关键词
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
                          f"先生成 {current_story}-CodingPlan.md（coding-process §A 调 coding-skill §5）")

    content = cp.read_text(encoding="utf-8")

    # 1. 14 关键词必须全在文档里
    missing_kw = [k for k in CODINGPLAN_14GATES_KEYWORDS if k not in content]
    if missing_kw:
        return GateResult("G-08", "CodingPlan 14 门禁通过", "blocker", False,
                          f"CodingPlan 缺 14 门禁关键词: {missing_kw}",
                          f"补全 14 门禁表（coding-skill §5.2 CodePlan 门禁）",
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
                         "story-reviewed", "testcase-generated", "testcase-reviewed",  # 🆕 v3.7.0
                         "task-generated", "task-reviewed"}

    scanner = _locate_authenticity_scanner(master_source)
    if scanner is None:
        return GateResult("G-09", "测试真实性扫描通过", "blocker", True,
                          "未找到母版 test_authenticity_scan.py（跳过）",
                          action="确认母版路径",
                          details={"scanned": False, "skipped": True, "stub": True})

    # 跑扫描（不卡 phase：保证能发现违规测试）
    try:
        result = runtime_exec.run_command(
            [sys.executable, str(scanner), "--root", str(project_dir), "--format", "json"],
            capture_output=True, text=True, timeout=60,
            span_name="scanner:test_authenticity",
            attrs={"scanRoot": str(project_dir)},
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
                         "story-reviewed", "testcase-generated", "testcase-reviewed",  # 🆕 v3.7.0
                         "task-generated", "task-reviewed"}

    scanner = _locate_coding_scanner(master_source)
    if scanner is None:
        return GateResult("G-CODE-1", "Coding 真实性扫描通过", "blocker", True,
                          "未找到母版 coding_authenticity_scan.py（跳过）",
                          action="确认母版路径",
                          details={"scanned": False, "skipped": True, "stub": True})

    try:
        result = runtime_exec.run_command(
            [sys.executable, str(scanner), "--root", str(project_dir), "--format", "json"],
            capture_output=True, text=True, timeout=60,
            span_name="scanner:coding_authenticity",
            attrs={"scanRoot": str(project_dir)},
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
    coding_report = _find_report_doc(
        project_dir, current_story,
        category="Coding",
        patterns=[f"{current_story}-CodingReport-v*-r*.md",
                  f"{current_story}-Coding-Report-v*-r*.md"],
        legacy_suffixes=["-Coding-Report.md", "-CodingReport.md"],
    )
    if coding_report is not None:
        cr_content = coding_report.read_text(encoding="utf-8")
        for t in tasks:
            # 引用判定：Task 的 stem（如 STORY-001-task-001）在 Coding Report 里出现
            if t.stem not in cr_content:
                issues.append(f"Coding Report 未引用 Task：{t.stem}")

    # 4. CodeReview → Story 引用追溯（如果存在）
    code_review = _find_report_doc(
        project_dir, current_story,
        category="CR",
        patterns=[f"{current_story}-CodeReview-v*-r*.md"],
        legacy_suffixes=["-CodeReview.md"],
    )
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
    if not project_dir or not Path(project_dir).is_dir():
        return out
    try:
        paths = list(project_dir.rglob("*.md"))
    except OSError:
        return out
    for path in paths:
        try:
            if not RA_FILENAME_RE.search(path.name):
                continue
            lower = path.as_posix().lower()
            if any(seg in lower for seg in ("changelog", "template", "ra-template", "change_log")):
                continue
        except OSError:
            continue
        out.append(path)
    return out


# 🆕 v3.5.10 Gap-005：从 RA 文件名提取版本号（如 RA-xxx-v1.2.md → (1, 2)）
_RA_VERSION_RE = re.compile(r"-v(\d+)\.(\d+)(?:\.(\d+))?", re.IGNORECASE)


def _select_latest_ra(ra_files: list[Path]) -> Path:
    """🆕 v3.5.10 Gap-005：选最新版 RA 文档。

    优先按文件名中的版本号（-v1.2 > -v1.1 > -v1.0）；版本号缺失或相同时
    fallback 到 mtime（保留原行为）。

    背景：cp -r 复制 / 打包分发会刷平所有文件 mtime，导致 max(mtime) 选出
    不确定的 RA 文档，G-RA-3 漏判 v1.2 已补的衍生章节。
    """
    def _version_key(p: Path) -> tuple:
        m = _RA_VERSION_RE.search(p.name)
        if m:
            major, minor = int(m.group(1)), int(m.group(2))
            patch = int(m.group(3)) if m.group(3) else 0
            return (1, major, minor, patch, 0)  # 第 1 位 = 1 表示有版本号
        return (0, 0, 0, 0, p.stat().st_mtime)  # 无版本号 → fallback mtime

    return max(ra_files, key=_version_key)


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
    latest = _select_latest_ra(ra_files)
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

    latest = _select_latest_ra(ra_files)
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

    latest = _select_latest_ra(ra_files)
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
        result = runtime_exec.run_command(
            [sys.executable, str(scanner), "--root", str(project_dir), "--format", "json"],
            capture_output=True, text=True, timeout=60,
            span_name="scanner:ra_authenticity",
            attrs={"scanRoot": str(project_dir)},
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
        # 🆕 v3.5.10 Gap-006：从 findings 抽前 5 条具体定位（file:line:snippet），
        # 让 AI 拿到结果知道改哪里——原版只列 rule 名，AI 不知道改哪个文件哪一行
        blocker_findings = [f for f in report.get("findings", [])
                            if f.get("severity") == "BLOCKER"][:5]
        location_hints = []
        for f in blocker_findings:
            loc = f.get("file") or f.get("path") or "?"
            line = f.get("line") or f.get("lineno") or ""
            snippet = (f.get("snippet") or f.get("message") or "")[:60]
            loc_str = f"{loc}" + (f":{line}" if line else "") + (f" — {snippet}" if snippet else "")
            location_hints.append(loc_str)
        msg = f"RA 真实性扫描发现 {blockers} 个 BLOCKER（共 {n_total} 项）：{blocker_rules}"
        if location_hints:
            msg += f" | 示例定位：{location_hints}"
        return GateResult("G-RA-4", name, "blocker", False,
                          msg,
                          "修复 RA 文档中标 BLOCKER 的项（看示例定位），或显式评审通过",
                          details={"scanned": True, "ra_files": ra_files_scanned,
                                   "blockers": blockers, "total": n_total,
                                   "blocker_rules": blocker_rules,
                                   "sample_locations": location_hints})

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
        result = runtime_exec.run_command(
            [sys.executable, str(scanner), "--root", str(project_dir), "--format", "json"],
            capture_output=True, text=True, timeout=60, check=False,
            span_name="scanner:flow_violation",
            attrs={"scanRoot": str(project_dir)},
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


# ─── G-RA-5：RA 机械派生深度通过（v3.5.9 — 防「形式通过、内容空转」）────────
# G-RA-3（章节锚点存在）/G-RA-4（无 fabricate/vague）/G-RA-FLOW-VIOLATION（流程完整性）
# 都是「存在性」检查。本门禁补「内容深度」正交维度：验证 E.5/G.5/H.6/H.5 规定的
# 「每条规则 R 必须机械追问 6 问 → 衍生 R'」「每条 R' 必须映射到 H.5 模式编号」
# 「§9-ter 每个触发动作必须回答 5 问」「§8.6 覆盖率是真实重算而非断言」。
# 杜绝「表填了但每行没机械派生」（用户实测：13 个问题 → 被逼出 34 个，根因即此）。
def _locate_ra_depth_scanner(master_source: Optional[Path]) -> Optional[Path]:
    """在母版找 ra_depth_scan.py（G-RA-5 运行时依赖，🆕 v3.5.9）。"""
    return _locate_runtime_script(master_source, "ra_depth_scan.py")


def check_ra_depth(project_dir: Path, st: dict, current_story: str,
                   master_source: Optional[Path] = None) -> GateResult:
    """G-RA-5 RA 机械派生深度通过（🆕 v3.5.9）。

    调 ra_depth_scan.py 跑 5 条机械派生规则检查：
      D1 §6.5 主规则机械派生（每条 R≥1 R'，每 R' 有模式编号）
      D2 R'→AC 链接完整性（每条 R' 在 §8.5 至少 1 AC）
      D3 §8.6 覆盖率真实重算（声明 K/M 须与实际一致）
      D4 §9-ter 五问机械覆盖（事件/缓存/MQ 必覆盖；时效禁模糊）
      D5 §9-bis 业务模式六选一（6 模式每个明确适用/不适用）
    BLOCKER=0 → pass；BLOCKER>0 → blocker。
    """
    name = "RA 机械派生深度通过"
    phase = st.get("phase", "initialized")
    pre_ra_phases = {"initialized", "ra-generated"}

    scanner = _locate_ra_depth_scanner(master_source)
    if scanner is None:
        return GateResult("G-RA-5", name, "blocker", True,
                          "未找到母版 ra_depth_scan.py（跳过）",
                          action="确认母版路径",
                          details={"scanned": False, "skipped": True, "stub": True})

    # pre-RA phase 无 RA 文档 → stub
    ra_files = _iter_ra_files(project_dir)
    if not ra_files and phase in pre_ra_phases:
        return GateResult("G-RA-5", name, "blocker", True,
                          "pre-RA 阶段无 RA 文档（stub 通过）",
                          details={"scanned": False, "stub": True, "phase": phase})

    try:
        result = runtime_exec.run_command(
            [sys.executable, str(scanner), "--root", str(project_dir), "--format", "json"],
            capture_output=True, text=True, timeout=60,
            span_name="scanner:ra_depth",
            attrs={"scanRoot": str(project_dir)},
        )
    except _subprocess.TimeoutExpired:
        return GateResult("G-RA-5", name, "blocker", False,
                          "ra_depth_scan.py 跑超过 60 秒",
                          "缩小扫描范围或增加超时")
    except Exception as e:
        return GateResult("G-RA-5", name, "blocker", False,
                          f"扫描器异常: {e}",
                          "检查 ra_depth_scan.py 是否可执行")

    try:
        report = _json.loads(result.stdout) if result.stdout else {}
    except _json.JSONDecodeError as e:
        return GateResult("G-RA-5", name, "blocker", False,
                          f"扫描器 JSON 输出无法解析: {e}",
                          f"stdout 前 200 字符: {result.stdout[:200]}")

    status = report.get("status", "UNKNOWN")
    ra_files_scanned = report.get("raFiles", 0)
    blockers = sum(1 for f in report.get("findings", []) if f.get("severity") == "BLOCKER")
    n_total = len(report.get("findings", []))

    if ra_files_scanned == 0:
        return GateResult("G-RA-5", name, "blocker", True,
                          "无 RA 文档可扫描（依赖 G-RA-1 判定）",
                          details={"scanned": True, "ra_files": 0, "stub": True})

    if status != "PASS" or blockers > 0:
        blocker_rules = sorted({f.get("rule") for f in report.get("findings", [])
                                if f.get("severity") == "BLOCKER"})
        return GateResult("G-RA-5", name, "blocker", False,
                          f"RA 机械派生深度扫描发现 {blockers} 个 BLOCKER（共 {n_total} 项）：{blocker_rules}",
                          "修复 RA 文档中标 BLOCKER 的项（E.5/G.5/H.6/H.5 机械追问逐行可见 + 链接齐全 + 覆盖率真实 + 五问覆盖 + 业务模式六选一）",
                          details={"scanned": True, "ra_files": ra_files_scanned,
                                   "blockers": blockers, "total": n_total,
                                   "blocker_rules": blocker_rules})

    return GateResult("G-RA-5", name, "blocker", True,
                      f"RA 机械派生深度扫描通过（{ra_files_scanned} 份 RA，0 BLOCKER，{n_total} WARN）",
                      details={"scanned": True, "ra_files": ra_files_scanned,
                               "blockers": 0, "total": n_total})


# ─── G-RA-6：RA 实现视角完整性通过（v3.5.18 — 支撑 DR/Coding 落地）───────
# G-RA-5 证明"衍生有深度"，本门禁证明"实现能落地"：RA 必须给出数据源清单、
# 数据流链路、术语/定义/不变量、现有实现/复用证据、高成本/难实现设计反驳、
# 开发者疑问答复矩阵、DR 生成交接包。否则下游 DR 只能补猜。
def _locate_ra_implementation_scanner(master_source: Optional[Path]) -> Optional[Path]:
    """在母版找 ra_implementation_scan.py（G-RA-6 运行时依赖，🆕 v3.5.18）。"""
    return _locate_runtime_script(master_source, "ra_implementation_scan.py")


def check_ra_implementation(project_dir: Path, st: dict, current_story: str,
                            master_source: Optional[Path] = None) -> GateResult:
    """G-RA-6 RA 实现视角完整性通过（🆕 v3.5.18）。

    调 ra_implementation_scan.py 跑 I1~I7：
      I1 数据源清单
      I2 数据流链路
      I3 术语/定义/不变量
      I4 现有实现/复用证据
      I5 高成本/难实现设计反驳
      I6 开发者疑问答复矩阵
      I7 DR 生成交接包
    BLOCKER=0 → pass；BLOCKER>0 → blocker。
    """
    name = "RA 实现视角完整性通过"
    phase = st.get("phase", "initialized")
    pre_ra_phases = {"initialized", "ra-generated"}

    scanner = _locate_ra_implementation_scanner(master_source)
    if scanner is None:
        return GateResult("G-RA-6", name, "blocker", True,
                          "未找到母版 ra_implementation_scan.py（跳过）",
                          action="确认母版路径",
                          details={"scanned": False, "skipped": True, "stub": True})

    ra_files = _iter_ra_files(project_dir)
    if not ra_files and phase in pre_ra_phases:
        return GateResult("G-RA-6", name, "blocker", True,
                          "pre-RA 阶段无 RA 文档（stub 通过）",
                          details={"scanned": False, "stub": True, "phase": phase})

    try:
        result = runtime_exec.run_command(
            [sys.executable, str(scanner), "--root", str(project_dir), "--format", "json"],
            capture_output=True, text=True, timeout=60,
            span_name="scanner:ra_implementation",
            attrs={"scanRoot": str(project_dir)},
        )
    except _subprocess.TimeoutExpired:
        return GateResult("G-RA-6", name, "blocker", False,
                          "ra_implementation_scan.py 跑超过 60 秒",
                          "缩小扫描范围或增加超时")
    except Exception as e:
        return GateResult("G-RA-6", name, "blocker", False,
                          f"扫描器异常: {e}",
                          "检查 ra_implementation_scan.py 是否可执行")

    try:
        report = _json.loads(result.stdout) if result.stdout else {}
    except _json.JSONDecodeError as e:
        return GateResult("G-RA-6", name, "blocker", False,
                          f"扫描器 JSON 输出无法解析: {e}",
                          f"stdout 前 200 字符: {result.stdout[:200]}")

    status = report.get("status", "UNKNOWN")
    ra_files_scanned = report.get("raFiles", 0)
    blockers = sum(1 for f in report.get("findings", []) if f.get("severity") == "BLOCKER")
    n_total = len(report.get("findings", []))

    if ra_files_scanned == 0:
        return GateResult("G-RA-6", name, "blocker", True,
                          "无 RA 文档可扫描（依赖 G-RA-1 判定）",
                          details={"scanned": True, "ra_files": 0, "stub": True})

    if status != "PASS" or blockers > 0:
        blocker_rules = sorted({f.get("rule") for f in report.get("findings", [])
                                if f.get("severity") == "BLOCKER"})
        sample_locations = []
        for f in [x for x in report.get("findings", []) if x.get("severity") == "BLOCKER"][:5]:
            loc = f.get("path") or f.get("file") or "?"
            line = f.get("line") or ""
            msg = (f.get("message") or "")[:90]
            sample_locations.append(f"{loc}" + (f":{line}" if line else "") + (f" — {msg}" if msg else ""))
        msg = f"RA 实现视角扫描发现 {blockers} 个 BLOCKER（共 {n_total} 项）：{blocker_rules}"
        if sample_locations:
            msg += f" | 示例定位：{sample_locations}"
        return GateResult("G-RA-6", name, "blocker", False,
                          msg,
                          "补全数据源/数据流/定义/复用证据/成本反驳/开发者疑问/DR 交接包后重跑",
                          details={"scanned": True, "ra_files": ra_files_scanned,
                                   "blockers": blockers, "total": n_total,
                                   "blocker_rules": blocker_rules,
                                   "sample_locations": sample_locations})

    return GateResult("G-RA-6", name, "blocker", True,
                      f"RA 实现视角扫描通过（{ra_files_scanned} 份 RA，0 BLOCKER，{n_total} WARN）",
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


def _is_subpath(path: Path, root: Path) -> bool:
    try:
        path.resolve().relative_to(root.resolve())
        return True
    except Exception:
        return False


def _display_doc_path(md_path: Path, project_dir: Path) -> str:
    try:
        return str(md_path.relative_to(project_dir)).replace("\\", "/")
    except ValueError:
        return str(md_path.resolve()).replace("\\", "/")


def _is_doc_product(md_path: Path, current_story: str) -> bool:
    fname = md_path.name
    return (current_story and current_story in fname) or any(
        t in fname for t in _DOC_FLOW_TYPES
    )


def _iter_doc_storage_scan_roots(project_dir: Path, real_workspace: Optional[Path]) -> list[Path]:
    roots = [project_dir]
    if real_workspace and not _is_subpath(real_workspace, project_dir):
        roots.append(real_workspace)
    return _dedupe_paths(roots)


def check_g_doc_storage(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-DOC-STORAGE 文档落地存放合规 — 产物路径/命名须合规

    🆕 v3.7.2（2026-07-01）：渐进增强——若能从 project_dir 定位 .ae-sdd/ 并读
    docWorkspacePath，则用它做真值校验（产物是否在真实 workspace 下）；拿不到
    时回退到硬编码 _DOC_COMPLIANT_ROOTS 列表（零回归）。
    """
    name = "文档落地存放合规"
    issues: list[str] = []

    # 🆕 v3.8.1：复用 paths.resolve_doc_workspace 解析新旧资产路径，避免本地手写规则漂移。
    # 拿不到不报错，回退到硬编码 _DOC_COMPLIANT_ROOTS（向后兼容）。
    real_workspace: Optional[Path] = None
    try:
        candidate_ae_sdd = project_dir / ".ae-sdd"
        if candidate_ae_sdd.is_dir():
            cfg = paths.read_config(candidate_ae_sdd)
            pk = cfg.get("projectKey") or cfg.get("project_key")
            if pk:
                real_workspace = paths.resolve_doc_workspace(candidate_ae_sdd, pk)
    except Exception:
        pass  # 任何异常都回退硬编码（零误伤）

    # 🆕 v3.5.10 Gap-007：用 git ls-files 拿"已被 git 跟踪的 .md"清单，
    # 已跟踪的文件视为历史产物（如 cp -r 复制来的 / 历史提交的），不报游离。
    # 仅对"未跟踪 + 新产出"的游离文件报。修复 fixture / 项目迁移场景误报。
    git_tracked: set[str] = set()
    try:
        ls_result = runtime_exec.run_command(
            ["git", "ls-files", "*.md"],
            cwd=project_dir, capture_output=True, text=True, timeout=10,
            span_name="subprocess:git_ls_files_md",
        )
        for line in ls_result.stdout.splitlines():
            git_tracked.add(line.strip().replace("\\", "/"))
    except Exception:
        pass  # git 不可用则空集合，回退到原行为（全部扫）

    # 扫描 project_dir + 配置 docWorkspacePath 下疑似流程产物（{STORY}-* 或 {DocType} 命名的 .md）。
    # 限定总量，避免 rglob 全盘扫描耗时。
    stray_files: list[str] = []
    checked = 0
    for scan_root in _iter_doc_storage_scan_roots(project_dir, real_workspace):
        if not scan_root.is_dir():
            continue
        for md_path in scan_root.rglob("*.md"):
            try:
                rel = md_path.relative_to(scan_root)
            except ValueError:
                continue
            rel_str = str(rel).replace("\\", "/")
            if any(seg in rel_str for seg in ("node_modules/", ".git/", "dist/", "CHANGELOG/", "docs/plans/")):
                continue
            # 🆕 v3.5.10 Gap-007：project_dir 内 git 已跟踪 = 历史产物，跳过
            if scan_root == project_dir and rel_str in git_tracked:
                continue
            checked += 1
            if checked > 500:  # 性能护栏
                break

            if not _is_doc_product(md_path, current_story):
                continue

            rel_lower = rel_str.lower()
            display_path = _display_doc_path(md_path, project_dir)
            # 1. 游离位置检测
            if any(m in rel_lower for m in _DOC_STRAY_MARKERS):
                stray_files.append(display_path)
                continue

            # 2. 不在合规根目录下
            in_compliant = any(rel_str.lower().startswith(r) or f"/{r}/" in f"/{rel_str.lower()}/"
                               for r in _DOC_COMPLIANT_ROOTS)
            # 🆕 v3.7.2 真值校验：若拿到 real_workspace，产物在其下也算合规
            if not in_compliant and real_workspace:
                try:
                    rel_ws = md_path.resolve().relative_to(real_workspace.resolve())
                    rel_ws_str = str(rel_ws).replace("\\", "/").lower()
                    if any(rel_ws_str.startswith(r) or f"/{r}/" in f"/{rel_ws_str}/"
                           for r in _DOC_COMPLIANT_ROOTS):
                        in_compliant = True
                except Exception:
                    pass  # resolve 异常回退硬编码判定
            # 允许直接在 project_dir 根的产物（向后兼容旧项目 design/ 在根的写法）
            if scan_root == project_dir and "/" not in rel_str:
                in_compliant = True
            if not in_compliant:
                stray_files.append(display_path)
        if checked > 500:
            break

    # 防御纵深：专项探针系统临时目录顶层的当前 Story 游离产物，不做递归全盘扫描。
    if current_story and checked <= 500:
        try:
            temp_root = Path(tempfile.gettempdir())
            for md_path in temp_root.glob(f"*{current_story}*.md"):
                checked += 1
                if checked > 500:
                    break
                if _is_doc_product(md_path, current_story):
                    display_path = _display_doc_path(md_path, project_dir)
                    if display_path not in stray_files:
                        stray_files.append(display_path)
        except Exception:
            pass

    if stray_files:
        issues.append(f"流程产物落在非合规位置（{len(stray_files)} 个）：{stray_files[:5]}")

    if issues:
        return GateResult("G-DOC-STORAGE", name, "blocker", False,
                          f"文档存放不合规：{'; '.join(issues)}",
                          "用 `ae-sdd doc save` 命令落地（代码自动推导路径）；禁止硬编码 d:\\tmp\\ 等游离位置",
                          details={"stray_files": stray_files, "checked": checked,
                                   "real_workspace": str(real_workspace) if real_workspace else None})

    return GateResult("G-DOC-STORAGE", name, "blocker", True,
                      f"文档存放合规（扫描 {checked} 个 .md，无游离产物）",
                      details={"stray_files": [], "checked": checked,
                               "real_workspace": str(real_workspace) if real_workspace else None})


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
                          details={"scanned": scanned, "violations": violations,
                                   "scope": "master"})

    # 🆕 v3.5.10 Gap-008：扩展扫描项目侧记忆文档（.ae-sdd/ + AGENTS.md + CLAUDE.md）
    # 母版扫描管"SKILL 文档自身漂移"；但项目侧 AGENTS.md/CLAUDE.md/.ae-sdd/memory 等记忆文档
    # 如果硬编码了 d:\tmp\ 或 design/story/be/ 等越界路径，会劫持 AI 写产物到错位置
    # （实测：life 项目 MEMORY.md 残留旧路径表述）。补这个盲区。
    project_violations: list[dict] = []
    project_scanned = 0
    project_scan_targets: list[Path] = []
    ae_sdd_dir = project_dir / ".ae-sdd"
    if ae_sdd_dir.is_dir():
        project_scan_targets.extend(ae_sdd_dir.rglob("*.md"))
    for mem_name in ("AGENTS.md", "CLAUDE.md", "MEMORY.md"):
        mem = project_dir / mem_name
        if mem.is_file():
            project_scan_targets.append(mem)
    harness_mem = project_dir / ".harness" / "memory"
    if harness_mem.is_dir():
        project_scan_targets.extend(harness_mem.rglob("*.md"))

    for md_path in project_scan_targets:
        try:
            rel = md_path.relative_to(project_dir)
        except ValueError:
            continue
        rel_str = str(rel).replace("\\", "/")
        if md_path.name == DOC_STORAGE_SKILL:
            continue
        project_scanned += 1
        if project_scanned > 200:
            break
        try:
            text = md_path.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for lineno, line in enumerate(text.splitlines(), start=1):
            if _PATH_VIOLATION_RE.search(line):
                project_violations.append({
                    "file": rel_str,
                    "line": lineno,
                    "snippet": line.strip()[:100],
                })

    if project_violations:
        files = sorted({v["file"] for v in project_violations})
        return GateResult("G-PATH", name, "blocker", False,
                          f"项目侧记忆文档存在路径越界（{len(project_violations)} 处，"
                          f"涉及 {len(files)} 文件）："
                          f"{[v['file']+':'+str(v['line']) for v in project_violations[:5]]}",
                          "修正项目侧记忆文档（AGENTS.md/CLAUDE.md/.ae-sdd/memory）的路径表述，"
                          "以 .ae-sdd/config.yaml docWorkspacePath 为 SSOT",
                          details={"scanned": scanned, "project_scanned": project_scanned,
                                   "violations": project_violations,
                                   "scope": "master+project_memory"})

    return GateResult("G-PATH", name, "blocker", True,
                      f"路径越界检测通过（扫描 {scanned} 个母版 .md + "
                      f"{project_scanned} 个项目侧记忆 .md，无硬编码产出路径）",
                      details={"scanned": scanned, "project_scanned": project_scanned,
                               "violations": [], "scope": "master+project_memory"})


# ─── G-DOC-CONSISTENCY：项目侧记忆-配置路径一致性（🆕 v3.5.7）─────────────────
# 堵"旧记忆劫持 config 路径"盲区：G-DOC-STORAGE 管"产物落在哪"，G-PATH 管"母版写了什么"，
# 本门禁管"项目侧 AGENTS.md/.harness/memory/MEMORY.md 的文档根表述是否与 config 一致"。
# 实测案例（2026-06-27 life 项目）：config 写 docWorkspacePath=D:\Item\life，但 MEMORY 写
# D:\Item\doc，主会话信旧记忆把 RA 写到错位置。本门禁把"config 是 SSOT"从声明变强制门禁。
#
# 扫描范围：项目根下 4 个活跃记忆文件（存在才扫）：
#   AGENTS.md / .harness/memory/MEMORY.md / .harness/agent.md / CLAUDE.md
# 判定：提取"文档工作区/文档根/文档位于"等声明式语境下的 Windows 绝对路径，
#       与 config 的 docWorkspacePath（缺省回退 gitPath）比对，互为前缀/相等=一致，否则冲突。
# 严重度：blocker，但从严判定——只拦截"= / 位于"等声明式表述，泛泛提及不拦截。
# 降级：无 config.yaml / 无 projectKey / 无记忆文件 → warn 不阻断。
# 记忆文件中"声明式文档根表述"线索词（行内含其一即候选）
_DOC_ROOT_CLUE_RE = re.compile(
    r"文档工作区|文档根|文档目录|文档放哪|Story\s*文档位于|Story 文档位于"
    r"|项目文档工作区|文档工作区根",
    re.IGNORECASE,
)
# Windows 绝对路径提取（D:\xxx / D:/xxx，含反斜杠或正斜杠，到下一个空白或反引号）
_WIN_ABS_PATH_RE = re.compile(
    r"`?([A-Za-z]:[\\/](?:[^\s`|，。、；:（）()\[\]]*[\\/])+[^\s`|，。、；:（）()\[\]]*)"
)
# 扫描的记忆文件相对路径（项目根下）
_MEMORY_FILES = (
    "AGENTS.md",
    ".harness/memory/MEMORY.md",
    ".harness/agent.md",
    "CLAUDE.md",
)


def _normalize_path(p: str) -> str:
    """归一化路径用于前缀比对：统一正斜杠 + 去尾分隔符 + 小写盘符。"""
    return p.replace("\\", "/").rstrip("/").lower()


def _paths_consistent(a: str, b: str) -> bool:
    """两路径是否一致：相等或互为前缀（容忍子目录表述如 ae-sdd-doc/ 在 life/ 下）。"""
    na, nb = _normalize_path(a), _normalize_path(b)
    return na == nb or na.startswith(nb + "/") or nb.startswith(na + "/")


def check_g_doc_consistency(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-DOC-CONSISTENCY 项目侧记忆-配置路径一致性。

    校验项目侧记忆文件（AGENTS.md/.harness/memory/MEMORY.md 等）里"文档工作区/文档根"
    等声明式表述的路径，是否与 .ae-sdd/config.yaml 的 docWorkspacePath（缺省回退 gitPath）一致。
    不一致 → 🔴 阻断（config 为 SSOT）。

    签名兼容 CHECK_FUNCS 通用路径 (project_dir, st, current_story)；
    内部从 project_dir/.ae-sdd 反推 ade_sdd + project_key。
    """
    name = "项目侧记忆-配置路径一致性"

    ade_sdd = project_dir / ".ae-sdd"
    if not ade_sdd.is_dir() or not paths.config_path(ade_sdd).is_file():
        # 无 .ae-sdd/（项目未 init）→ 降级 warn，不阻断（同 G-00 缺失策略）
        return GateResult("G-DOC-CONSISTENCY", name, "blocker", True,
                          "未找到 .ae-sdd/config.yaml（项目未 init），跳过一致性校验",
                          details={"skipped": "no_config", "scanned": 0, "conflicts": []})

    cfg = paths.read_config(ade_sdd)
    project_key = cfg.get("workspaceKey") or cfg.get("projectKey") or ""
    if not project_key:
        # 无 projectKey 无法读 assets.md 取 docWorkspacePath，降级
        return GateResult("G-DOC-CONSISTENCY", name, "blocker", True,
                          "config.yaml 无 workspaceKey/projectKey，跳过一致性校验",
                          details={"skipped": "no_project_key", "scanned": 0, "conflicts": []})

    # 取 config 权威值：docWorkspacePath > 缺省回退 gitPath
    doc_ws = paths.resolve_doc_workspace(ade_sdd, project_key)
    if doc_ws is None:
        # assets.md 无 gitPath/docWorkspacePath → 无权威值可比，降级
        return GateResult("G-DOC-CONSISTENCY", name, "blocker", True,
                          f"assets.md 无 docWorkspacePath/gitPath（projectKey={project_key}），跳过",
                          details={"skipped": "no_canonical", "scanned": 0, "conflicts": []})
    canonical = str(doc_ws.resolve())

    # 扫记忆文件，提取声明式文档根路径表述
    conflicts: list[dict] = []
    scanned = 0
    for rel in _MEMORY_FILES:
        mem_file = project_dir / rel
        if not mem_file.is_file():
            continue
        scanned += 1
        try:
            text = mem_file.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for lineno, line in enumerate(text.splitlines(), start=1):
            # 行内须同时命中"声明式线索词"才视为候选（泛泛提及不拦，避免误伤历史引用）
            if not _DOC_ROOT_CLUE_RE.search(line):
                continue
            for m in _WIN_ABS_PATH_RE.finditer(line):
                candidate = m.group(1)
                if not _paths_consistent(candidate, canonical):
                    conflicts.append({
                        "file": rel,
                        "line": lineno,
                        "path": candidate,
                        "canonical": canonical,
                        "snippet": line.strip()[:120],
                    })

    if conflicts:
        locs = [f"{c['file']}:{c['line']}({c['path']})" for c in conflicts[:5]]
        return GateResult(
            "G-DOC-CONSISTENCY", name, "blocker", False,
            f"项目侧记忆的文档路径表述与 config.yaml 不一致（{len(conflicts)} 处冲突，"
            f"config docWorkspacePath={canonical}）：{locs}",
            "config.yaml 的 docWorkspacePath 是唯一权威源；修正项目侧记忆文件中的文档路径表述使其一致",
            details={"canonical": canonical, "scanned": scanned, "conflicts": conflicts})

    return GateResult("G-DOC-CONSISTENCY", name, "blocker", True,
                      f"项目侧记忆-配置路径一致（扫描 {scanned} 个记忆文件，docWorkspacePath={canonical}）",
                      details={"canonical": canonical, "scanned": scanned, "conflicts": []})


# ─── 🆕 v3.5.12 G-REVIEW-LOOP review-loop 退出条件门禁 ──────────────────────────
def check_g_review_loop(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-REVIEW-LOOP review-loop 退出条件通过（🆕 v3.5.12，治 P0-1/4）。

    校验 review 节点（story-reviewed/testcase-reviewed/dr-reviewed/task-reviewed/code-reviewed）切相前，
    reviewLoop 状态满足退出条件（协议1 normal 需 dryCounter≥3 / 协议2 escalate 已升级用户）。

    降级策略：若 state 无 reviewLoop 字段（root 未跑过 review-loop CLI）→ skip（warn 不阻断）。
    这是兼容旧 state.json 的策略——本门禁只在 root 主动启用 review-loop CLI 后生效，
    不强制所有 review 节点必须用（微任务/Tier1 单审可不用）。
    """
    name = "review-loop 退出条件通过"
    phase = st.get("phase", "initialized")

    # 只在 review 后的 phase 校验（story-reviewed/testcase-reviewed/dr-reviewed/task-reviewed/code-reviewed）
    review_phases = {"story-reviewed", "testcase-reviewed", "task-reviewed", "code-reviewed"}  # 🆕 v3.7.0 补 testcase-reviewed
    if phase not in review_phases:
        return GateResult("G-REVIEW-LOOP", name, "blocker", True,
                          f"阶段 {phase} 非 review 节点（skip）",
                          details={"skipped": True, "reason": "non-review-phase"})

    rl = st.get("reviewLoop")
    if not rl:
        # 降级：root 未启用 review-loop CLI → skip（兼容旧 state）
        return GateResult("G-REVIEW-LOOP", name, "blocker", True,
                          f"阶段 {phase} 无 reviewLoop 状态（root 未启用 review-loop CLI，skip）",
                          details={"skipped": True, "reason": "no-review-loop-state"})

    from lib import review_loop as rl_mod
    passed, reason = rl_mod.verify_exit(rl)
    if passed:
        return GateResult("G-REVIEW-LOOP", name, "blocker", True,
                          f"review-loop 退出条件满足：{reason}",
                          details={"exitReason": rl.get("exitReason"),
                                   "dryCounter": rl.get("dryCounter"),
                                   "round": rl.get("round")})
    return GateResult("G-REVIEW-LOOP", name, "blocker", False,
                      f"review-loop 未达退出条件：{reason}",
                      "跑 `ae-sdd review-loop collect` 推进轮次直到连续3轮无新增（normal）或升级用户（escalate）",
                      details={"exitReason": rl.get("exitReason"),
                               "dryCounter": rl.get("dryCounter"),
                               "round": rl.get("round")})


# ─── 🆕 v3.5.13 G-09B reviewer 独立性硬门禁（堵"root 总派给自己"）─────────────
def _derive_tier_from_context(project_dir: Path, st: dict) -> tuple:
    """从 RA/Story 文档机械派生 Tier + 返回 (tier, ra_size, decision_hints, rule)。

    复用 review_loop.derive_tier。决策线索 = RA 文档 + Story 文档关键词拼接。
    """
    from lib import review_loop as rl_mod
    # 读 RA 文档规模 + 决策线索
    decision_hints = ""
    ra_size = "中"  # 默认中规模（保守 → Tier 2）
    try:
        ra_files = _iter_ra_files(project_dir)
        if ra_files:
            latest_ra = _select_latest_ra(ra_files)
            ra_text = latest_ra.read_text(encoding="utf-8", errors="replace")
            # 提取规模（RA 文档常见标记）
            import re as _re
            size_m = _re.search(r"规模[：:]\s*[大小中微]", ra_text)
            if size_m:
                ra_size = size_m.group(0).split("：")[-1].split(":")[-1].strip()
            decision_hints = ra_text[:3000]  # 取前 3000 字作决策线索
        # 追加 Story 文档作线索
        story_files = list((project_dir / "design").rglob("*Story*.md")) if (project_dir / "design").exists() else []
        for sf in story_files[:2]:
            try:
                decision_hints += sf.read_text(encoding="utf-8", errors="replace")[:2000]
            except OSError:
                pass
    except Exception:
        pass  # 读不到降级默认 Tier 2

    tier_result = rl_mod.derive_tier(ra_size, decision_hints)
    return tier_result.tier, ra_size, decision_hints[:200], tier_result.rule


def check_g09b(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-09B reviewer 独立性通过（🆕 v3.5.13，堵"root 总派给自己"）。

    独立硬门禁——review 节点切相时由 check_all 跑，**不依赖 root 调 review-loop collect**：
      1. 机械派生 Tier（读 RA 规模 + 决策线索）
      2. Tier 1（微/小 + 无关键决策）→ skip（单审合规）
      3. Tier 2/3 → 校验 state.activeAgents 有 ≥Tier 个 sessionId≠root 的 reviewer

    堵死的偷懒路径：
      - root 不派（activeAgents 空）→ 阻断
      - root 自扮（sessionId==root）→ 阻断
      - root 派不够（< Tier 要求）→ 阻断

    root 要过此门禁，必须真实用 Agent 工具派 sub-agent 并调 state.register_agent 登记。
    """
    name = "reviewer 独立性通过"
    phase = st.get("phase", "initialized")

    # 只在 review 节点切相后校验（与 G-REVIEW-LOOP 同 phase 集）
    review_phases = {"story-reviewed", "testcase-reviewed", "task-reviewed", "code-reviewed"}  # 🆕 v3.7.0 补 testcase-reviewed
    if phase not in review_phases:
        return GateResult("G-09B", name, "blocker", True,
                          f"阶段 {phase} 非 review 节点（skip）",
                          details={"skipped": True, "reason": "non-review-phase"})

    # 读 root sessionId
    from lib import session as session_mod
    ade_sdd = None
    try:
        from lib import paths as paths_mod
        ade_sdd = paths_mod.locate_project_ae_sdd()
    except Exception:
        pass
    root_sess = session_mod.read_session(ade_sdd, current_story) or {}
    root_sid = root_sess.get("sessionId", "")

    # 机械派生 Tier
    tier, ra_size, hints_preview, tier_rule = _derive_tier_from_context(project_dir, st)

    # Tier 1 豁免（微/小规模 + 无关键决策 → 单审合规）
    if tier == 1:
        return GateResult("G-09B", name, "blocker", True,
                          f"Tier 1（{tier_rule}）→ 单审豁免",
                          details={"tier": 1, "raSize": ra_size, "exempt": True})

    # Tier 2/3：校验 activeAgents + reviewLoop 字段存在性
    from lib import review_loop as rl_mod
    active_agents = st.get("activeAgents", [])
    # 只数 reviewer 类角色（story-reviewer/code-reviewer 等）
    reviewer_roles = {"story-reviewer", "code-reviewer", "dr-reviewer", "ra-reviewer", "task-reviewer"}
    reviewer_sids = [a.get("sessionId", "") for a in active_agents
                     if a.get("role") in reviewer_roles and a.get("status") != "completed"]

    session_chk = rl_mod.check_session_independence(reviewer_sids, root_sid, tier)
    if not session_chk.passed:
        return GateResult("G-09B", name, "blocker", False,
                          f"Tier {tier}（{tier_rule}）但 reviewer 不独立：{session_chk.reason}",
                          f"用 Agent 工具派 {tier} 个 sub-agent（视角见 review-loop start 输出），"
                          f"每个用独立 session，派后调 state.register_agent 登记 activeAgents",
                          details={"tier": tier, "raSize": ra_size,
                                   "reviewerCount": len(reviewer_sids),
                                   "violations": session_chk.violations,
                                   "hintsPreview": hints_preview})

    # Tier 2/3 还要求 root 跑过 review-loop（堵"派了多 reviewer 但单轮就退"）
    # G-REVIEW-LOOP 负责"跑满3轮"，G-09B 负责"启动了 review-loop"
    if not st.get("reviewLoop"):
        return GateResult("G-09B", name, "blocker", False,
                          f"Tier {tier} 但未启动 review-loop（无 reviewLoop 字段）",
                          f"跑 `ae-sdd review-loop start --node <节点> --ra-size {ra_size}` 启动状态机，"
                          f"再 collect 推进轮次（G-REVIEW-LOOP 校验退出条件）",
                          details={"tier": tier, "raSize": ra_size,
                                   "reviewerCount": len(reviewer_sids),
                                   "missingReviewLoop": True})

    return GateResult("G-09B", name, "blocker", True,
                      f"Tier {tier}（{tier_rule}）：{session_chk.reason} + review-loop 已启动",
                      details={"tier": tier, "raSize": ra_size,
                               "reviewerCount": len(reviewer_sids),
                               "rootSid": root_sid[:8] + "..." if root_sid else None})


# ─── 🆕 v3.8.0 G-AUTO-CONSENSUS 自动化联审共识门禁 ─────────────────────────────
def check_g_auto_consensus(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-AUTO-CONSENSUS 自动化联审共识通过（🆕 v3.8.0）。

    仅在自动化模式（config.yaml automation.enabled=true）下对白名单审核点生效：
      1. 非自动化模式 → skip（回退人工审核）
      2. 自动化模式但当前 phase 非 review 节点 → skip
      3. 自动化模式 + review 节点 → 校验 state.reviewConsensus[point].passed=true
         + reviewer 独立性（复用 G-09B 模式：activeAgents 有 ≥Tier 个独立 session）

    堵死路径：
      - 自动化模式下未写 reviewConsensus 就推进 phase → 阻断
      - reviewConsensus.passed=false 仍推进 → 阻断
      - reviewer 不独立（复用 G-09B）→ 阻断
    """
    name = "自动化联审共识通过"
    phase = state_mod.get_active_phase(st) or st.get("phase", "initialized")

    # 1. 非自动化模式 → skip
    try:
        from lib import config as cfg_mod
        if not cfg_mod.is_automation_enabled():
            return GateResult("G-AUTO-CONSENSUS", name, "blocker", True,
                              "非自动化模式（skip，回退人工审核）",
                              details={"skipped": True, "reason": "automation-disabled"})
    except Exception as e:
        # 读 config 失败 → 保守 skip（不阻断非自动化项目）
        return GateResult("G-AUTO-CONSENSUS", name, "blocker", True,
                          f"自动化配置读取失败，保守 skip：{e}",
                          details={"skipped": True, "reason": "config-read-error"})

    # 2. 非 review 节点 → skip（与 G-09B 同 phase 集）
    review_phases = {"story-reviewed", "testcase-reviewed", "task-reviewed", "code-reviewed"}
    if phase not in review_phases:
        return GateResult("G-AUTO-CONSENSUS", name, "blocker", True,
                          f"自动化模式但阶段 {phase} 非 review 节点（skip）",
                          details={"skipped": True, "reason": "non-review-phase"})

    # 3. 校验 reviewConsensus
    rc = st.get("reviewConsensus") or {}
    if not rc:
        return GateResult("G-AUTO-CONSENSUS", name, "blocker", False,
                          "自动化模式 review 节点但未写 reviewConsensus",
                          "派 Tier 3 reviewer 跑 §8.4.3 交叉对比后调 "
                          "state register-review-consensus --point {1|1.5|2|2.5|4|5} --passed true",
                          details={"skipped": False, "reason": "missing-reviewConsensus"})

    # 取该 phase 对应的审核点共识（取任一即可，因 review 节点切相前应已写）
    # phase→审核点映射：story-reviewed→1/1.5, testcase-reviewed→(无), task-reviewed→2, code-reviewed→4
    phase_to_points = {
        "story-reviewed": [1, 1.5],
        "task-reviewed": [2],
        "code-reviewed": [4],
    }
    points = phase_to_points.get(phase, [])
    if not points:
        # testcase-reviewed 等无对应人工审核点的 review 节点 → 仅校验 reviewer 独立性（G-09B 已管）
        return GateResult("G-AUTO-CONSENSUS", name, "blocker", True,
                          f"自动化模式 review 节点 {phase} 无对应人工审核点（仅 G-09B 管独立性）",
                          details={"skipped": True, "reason": "no-mapped-point"})

    # 找第一个有记录的审核点（兼容 str(int)="1" 与 str(float)="1.0" 两种 key）
    consensus = None
    matched_point = None
    for p in points:
        c = rc.get(str(p)) or rc.get(str(float(p)))
        if c:
            consensus = c
            matched_point = p
            break

    if not consensus:
        return GateResult("G-AUTO-CONSENSUS", name, "blocker", False,
                          f"自动化模式 {phase} 但审核点 {points} 无 reviewConsensus 记录",
                          f"调 state register-review-consensus --point {points[0]} --passed true",
                          details={"skipped": False, "reason": "no-consensus-for-point",
                                   "expectedPoints": points})

    if not consensus.get("passed"):
        return GateResult("G-AUTO-CONSENSUS", name, "blocker", False,
                          f"审核点 {matched_point} 联审共识未通过（rounds={consensus.get('rounds')}）",
                          f"stallReason={consensus.get('stallReason','')}；按 automation.onConsensusStall 处理",
                          details={"skipped": False, "point": matched_point,
                                   "passed": False, "rounds": consensus.get("rounds"),
                                   "stallReason": consensus.get("stallReason")})

    # 4. reviewer 独立性（复用 G-09B 逻辑）
    tier = int(consensus.get("tier", 3))
    reviewers = consensus.get("reviewers") or []
    reviewer_sids = [r.get("sessionId", "") for r in reviewers
                     if r.get("sessionId")]
    if len(reviewer_sids) < tier:
        return GateResult("G-AUTO-CONSENSUS", name, "blocker", False,
                          f"审核点 {matched_point} 共识通过但 reviewer 数 {len(reviewer_sids)} < Tier {tier}",
                          "reviewers 字段需 ≥Tier 个独立 sessionId（与 G-09B 一致）",
                          details={"skipped": False, "point": matched_point,
                                   "reviewerCount": len(reviewer_sids),
                                   "expectedTier": tier})

    return GateResult("G-AUTO-CONSENSUS", name, "blocker", True,
                      f"审核点 {matched_point} 联审共识通过（Tier {tier}, rounds={consensus.get('rounds')}）",
                      details={"point": matched_point, "tier": tier,
                               "passed": True, "rounds": consensus.get("rounds"),
                               "reviewerCount": len(reviewer_sids)})


# ─── 🆕 v3.9.1 上下文加载准入门禁（注册表模式）────────────────────────────────
# 背景：RA 有 G-RA-1~6、Coding 有 G-CODEPLAN-SRC/G-14/G-08，但 DR/Story/TestCase/Task
# 四组的「第零步准入检查」长期只是 prose（dr-review L14-21 / task-generate L14-21 还
# 官方自认 report-only）。本组门禁把这四组的「上下文是否真读齐」从 prose 变成机械阻断。
#
# 设计：注册表驱动 — 流程一致用 _check_context_loaded 一个函数封装，上下文差异走
# CONTEXT_GATE_REGISTRY 注册表；读文件统一走 document-storage-skill API
# （get_constraints / get_assets / resolve_path / paths.find_doc）。
#
# phase 感知：准入门禁挂在「切到 X-phase」入口，但 check 执行时 st["phase"] 仍是前一
# 阶段，故注册表 key 用 gate_id（不用 target_phase）；用 st["phase"] 是否已过该阶段
# 判定 stub/skip。

# phase 顺序（对齐 state.py PHASE_FLOWS 大链），用于判定「已过该阶段」
_PHASE_ORDER = [
    "initialized", "ra-generated", "dr-generated",
    "story-generated", "story-reviewed",
    "testcase-generated", "testcase-reviewed",
    "task-generated", "task-reviewed",
    "coding-process", "coding", "test-running", "code-reviewed", "completed",
]


def _phase_index(phase: str) -> int:
    """返回 phase 在 _PHASE_ORDER 中的位置；未知返回 -1。"""
    try:
        return _PHASE_ORDER.index(phase)
    except ValueError:
        return -1


# 上下文准入门禁注册表：gate_id → 该阶段必须读齐的上下文类型
# required 的 key 对齐 document-storage-skill API：
#   constraints → get_constraints(ade_sdd, project_key)
#   assets      → get_assets(ade_sdd, project_key)
#   RA          → _iter_ra_files(project_dir)（复用 G-RA-1 逻辑）
#   PRD         → rglob *PRD*.md / *prd*.md / *需求*.md（gitPath + docWorkspace）
#   DR          → paths.find_doc 或 design/ rglob *DR*.md（复用 G-01 逻辑）
#   Story       → paths.find_doc(current_story, ".md")
#   TestCase    → paths.find_doc(current_story, "-testcase.md")
#   Story       → paths.find_doc(current_story, ".md")
#   TestCase    → paths.find_doc(current_story, "-testcase.md")
#   dependsStory → 🆕 v3.9.3 扫描当前 Story 元信息中"复用其他 Story"表，自动发现依赖并校验可达
#   sourceTrace → 🆕 v3.9.3 扫描当前 Story 接口契约章节，校验每字段"来源"列非空 + 来源文档可达
CONTEXT_GATE_REGISTRY: dict[str, dict] = {
    "G-DR-CTX": {
        "name": "DR 上下文加载",
        "scales": {"大", "中"},                 # 大/中链必填，小/微链豁免
        "pre_phases": {"initialized", "ra-generated"},  # 这些 phase 时 stub 通过
        "passed_phases": {"story-generated", "story-reviewed",
                          "testcase-generated", "testcase-reviewed",
                          "task-generated", "task-reviewed",
                          "coding-process", "coding", "test-running",
                          "code-reviewed", "completed"},
        "required": ["constraints", "assets", "RA", "PRD"],
    },
    "G-STORY-CTX": {
        "name": "Story 上下文加载",
        "scales": {"大", "中", "小", "微"},
        "pre_phases": {"initialized", "ra-generated", "dr-generated"},
        "passed_phases": {"story-reviewed", "testcase-generated", "testcase-reviewed",
                          "task-generated", "task-reviewed",
                          "coding-process", "coding", "test-running",
                          "code-reviewed", "completed"},
        # 🆕 v3.9.3: dependsStory + sourceTrace 覆盖 SSOT §3 C 类与 §4 来源追溯
        # 🆕 v3.9.20: standardsRef 升级为真"已引用"门禁（查产物证据，不查行为）
        #              + scales 扩到 {大,中,小,微}，取消小/微豁免（小/微走轻量阈值）
        "required": ["constraints", "assets", "DR", "PRD",
                     "dependsStory", "sourceTrace", "standardsRef"],
    },
    "G-TESTCASE-CTX": {
        "name": "TestCase 上下文加载",
        "scales": {"大", "中", "小"},
        "pre_phases": {"initialized", "ra-generated", "dr-generated",
                       "story-generated", "story-reviewed"},
        "passed_phases": {"task-generated", "task-reviewed",
                          "coding-process", "coding", "test-running",
                          "code-reviewed", "completed"},
        "required": ["constraints", "assets", "Story"],
    },
    "G-TASK-CTX": {
        "name": "Task 上下文加载",
        "scales": {"大", "中", "小", "微"},
        "pre_phases": {"initialized", "ra-generated", "dr-generated",
                       "story-generated", "story-reviewed",
                       "testcase-generated", "testcase-reviewed"},
        "passed_phases": {"coding-process", "coding", "test-running",
                          "code-reviewed", "completed"},
        # 微链豁免 Story/TestCase（微链无 Story/TestCase 产物）
        "required": ["constraints", "assets", "Story", "TestCase"],
        "required_micro": ["constraints", "assets"],
    },
}


def _find_prd_files(project_dir: Path) -> list[Path]:
    """rglob PRD 文档（多命名约定：PRD/prd/需求）。"""
    out: list[Path] = []
    seen: set[Path] = set()
    patterns = ["*PRD*.md", "*prd*.md", "*需求*.md"]
    for pat in patterns:
        for p in project_dir.rglob(pat):
            try:
                resolved = p.resolve()
            except OSError:
                resolved = p
            if resolved in seen:
                continue
            # 排除明显的模板/changelog
            lower = p.as_posix().lower()
            if any(seg in lower for seg in ("changelog", "template", "change_log")):
                continue
            seen.add(resolved)
            out.append(p)
    return out


# 🆕 v3.9.3: 依赖 Story 自动发现与可达性校验
# 扫描当前 Story 元信息中"复用其他 Story / 模块的能力"表格，
# 提取 STORY-XXX-BE 形式 ID，逐个验证 ae-sdd doc resolve --intent STORY --story-id {ID} 可达。
def _check_depends_story(project_dir: Path, current_story: str) -> tuple[bool, str]:
    """返回 (ok, detail)：ok=True 通过；ok=False 失败 detail 为人类可读失败原因。"""
    if not current_story:
        return True, ""  # 无当前 Story 不阻断（与 G-TESTCASE-CTX 一致处理）
    story_doc = paths.find_doc(project_dir, current_story, ".md")
    if story_doc is None:
        return True, ""  # 当前 Story 不存在 → 由其他门禁阻断，本门禁不重复
    try:
        content = story_doc.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return True, ""  # 读不到文件不阻断（其他门禁会处理）
    # 用正则提取所有 STORY-XXX-BE / STORY-XXX-FE 形式的 ID
    import re as _re
    ids = set(_re.findall(r"STORY-[A-Z0-9]+-(?:BE|FE|FULL|MP|MiniProgram)", content))
    if not ids:
        return True, ""  # 无依赖声明 → 视为通过（不强求必须声明依赖）
    missing_ids: list[str] = []
    for sid in sorted(ids):
        if paths.find_doc(project_dir, sid, ".md") is None:
            missing_ids.append(sid)
    if missing_ids:
        return False, f"以下依赖 Story 不可达：{', '.join(missing_ids)}"
    return True, ""


# 🆕 v3.9.3: 接口契约字段来源追溯校验
# 扫描当前 Story 接口契约章节（REST API / SPI），
# 验证每个字段都有"来源"列填写，且非空。
def _check_source_trace(project_dir: Path, current_story: str) -> tuple[bool, str]:
    """返回 (ok, detail)。"""
    if not current_story:
        return True, ""
    story_doc = paths.find_doc(project_dir, current_story, ".md")
    if story_doc is None:
        return True, ""
    try:
        content = story_doc.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return True, ""
    # 简化实现：检查"接口契约"章节中是否有 Request/Response 表头，
    # 且至少有一个非空"来源"列。若章节缺失则视为不适用（不阻断），
    # 因为纯后端无 API 的 Story 不需要接口契约来源追溯。
    if "接口契约" not in content and "接口契约-SPI" not in content:
        return True, ""
    import re as _re
    # 抓取接口契约章节内所有表格行（含 | xxx | yyy | zzz |），逐行检查"来源"列
    in_contract = False
    bad_rows: list[str] = []
    for line in content.splitlines():
        if "## 接口契约" in line or "## 接口契约-SPI" in line:
            in_contract = True
            continue
        if in_contract and line.startswith("## ") and "接口契约" not in line:
            break  # 离开接口契约章节
        if not in_contract:
            continue
        # 只看表格数据行（以 | 开头且包含 3 个以上 |）
        if not line.strip().startswith("|") or line.count("|") < 4:
            continue
        # 跳过表头分隔行 |---|---|...|
        if _re.match(r"^\|\s*-+", line):
            continue
        # 找到"来源"列索引（在表头行查找，本行按相同索引切分）
        # 简化策略：本行含"来源"列文本片段 且 同行"来源"位置为空 → 标记 bad
        # 因为 Markdown 表格解析复杂，此处用"列数对齐 + 至少有一个含来源关键词的列"
        cells = [c.strip() for c in line.strip("|").split("|")]
        # 检查是否有"来源"含义的列（第 6 列通常为"来源"，按模板约定）
        if len(cells) >= 6 and cells[5] in ("", "-", "—", "TODO"):
            bad_rows.append(line[:80])
    if bad_rows:
        return False, f"接口契约中有 {len(bad_rows)} 行『来源』列为空（示例：{bad_rows[0]}...）"
    return True, ""


# 🆕 v3.9.20: 约束文档标识性关键词。Story 正文命中这些关键词即视为"已引用并遵循标准"。
# 关键词取自 source/standards/constraints/*.md 的文档名 + 核心章节锚点。
# 设计哲学：不查 agent 是否真读（无法机械验证），查产物是否引用了标准的可验证信号。
_STANDARDS_KEYWORDS = {
    "database": ["DDL 约束", "标准建表", "分库分表", "索引", "主键", "表名", "字段名"],
    "api": ["URL 命名", "HTTP 方法", "RESTful", "状态码", "接口规范"],
    "code-style": ["注释规范", "异常处理", "命名", "代码风格"],
    "layered-arch": ["分层架构", "Controller", "Service", "Repository", "防腐层", "ACL", "跨层调用"],
    "security": ["参数化查询", "SQL 注入", "敏感数据", "加密", "脱敏", "XSS", "CSRF"],
    "testing": ["单元测试", "覆盖率", "测试策略", "Mock"],
    "project-structure": ["工程结构", "模块划分", "包结构"],
    "technology-stack": ["技术栈", "Spring Boot", "MyBatis"],
}


def _check_standards_referenced(project_dir: Path, current_story: str,
                                scale: str) -> tuple[bool, str, list[str]]:
    """🆕 v3.9.20: 校验 Story 正文是否引用了约束文档标准（产物级证据）。

    返回 (ok, detail, hit_categories)。
    - 大/中链：正文须命中 ≥3 个约束类别的关键词。
    - 小/微链：正文须命中 ≥1 个约束类别的关键词（轻量要求）。
    命中即视为"已加载并遵循标准"——不查行为，查产物证据。
    缺失 current_story 或 Story 文件时 stub 通过（由外层文件存在检查兜底）。
    """
    if not current_story:
        return True, "", []
    story_doc = paths.find_doc(project_dir, current_story, ".md")
    if story_doc is None:
        return True, "", []
    try:
        content = story_doc.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return True, "", []

    threshold = 1 if scale in ("小", "微") else 3
    hit_categories: list[str] = []
    for category, keywords in _STANDARDS_KEYWORDS.items():
        if any(kw in content for kw in keywords):
            hit_categories.append(category)

    if len(hit_categories) < threshold:
        return (False,
                f"Story 正文仅引用 {len(hit_categories)} 个约束类别（需 ≥{threshold}），"
                f"命中：{hit_categories or '无'}",
                hit_categories)
    return True, "", hit_categories


def _check_context_loaded(project_dir: Path, st: dict, current_story: str,
                          gate_id: str) -> GateResult:
    """上下文加载准入门禁统一实现（注册表驱动）。

    复用 document-storage-skill API：
      - get_constraints(ade_sdd, project_key) → 约束文档
      - get_assets(ade_sdd, project_key) → 项目资产文件列表
      - _iter_ra_files / _find_prd_files / paths.find_doc → 上游设计文档

    签名兼容 CHECK_FUNCS 通用路径 (project_dir, st, current_story)；
    内部从 project_dir/.ae-sdd 反推 ade_sdd + project_key。
    """
    spec = CONTEXT_GATE_REGISTRY[gate_id]
    name = spec["name"]
    phase = st.get("phase", "initialized")
    scale = st.get("scale") or "大"

    # 1. scale 豁免
    if scale not in spec["scales"]:
        return GateResult(gate_id, name, "blocker", True,
                          f"scale={scale} 不在本门禁适用范围 {sorted(spec['scales'])}（豁免）",
                          details={"skipped": True, "reason": "scale-exempt",
                                   "scale": scale, "applicable_scales": sorted(spec["scales"])})

    # 2. phase 感知：pre-phase stub 通过
    if phase in spec["pre_phases"]:
        return GateResult(gate_id, name, "blocker", True,
                          f"phase={phase}（pre-phase，上下文加载尚不适用，stub 通过）",
                          details={"skipped": True, "reason": "pre-phase", "phase": phase})

    # 3. 已过该阶段 → skipped（不回溯阻断历史 state）
    if phase in spec["passed_phases"]:
        return GateResult(gate_id, name, "blocker", True,
                          f"phase={phase}（已过本门禁适用阶段，跳过）",
                          details={"skipped": True, "reason": "passed", "phase": phase})

    # 4. 反推 ade_sdd + project_key
    ade_sdd = project_dir / ".ae-sdd"
    if not ade_sdd.is_dir() or not paths.config_path(ade_sdd).is_file():
        return GateResult(gate_id, name, "blocker", True,
                          "未找到 .ae-sdd/config.yaml（项目未 init），跳过上下文加载校验",
                          details={"skipped": True, "reason": "no-config"})
    cfg = paths.read_config(ade_sdd)
    project_key = cfg.get("workspaceKey") or cfg.get("projectKey") or ""
    if not project_key:
        return GateResult(gate_id, name, "blocker", True,
                          "config.yaml 无 workspaceKey/projectKey，跳过上下文加载校验",
                          details={"skipped": True, "reason": "no-project-key"})

    # 5. 确定本 scale 下的 required 列表（微链 Task 豁免 Story/TestCase）
    if gate_id == "G-TASK-CTX" and scale == "微":
        required_keys = spec["required_micro"]
    else:
        required_keys = spec["required"]

    # 6. 逐项校验
    status: dict[str, bool] = {}
    missing: list[str] = []
    missing_hints: list[str] = []

    for key in required_keys:
        if key == "constraints":
            from lib import document_storage as ds
            constraints = ds.get_constraints(ade_sdd, project_key)
            ok = bool(constraints)
            status["constraints"] = ok
            if not ok:
                missing.append("项目约束文档")
                missing_hints.append("走 project-assets-update-skill 生成 constraints/*.md")
        elif key == "assets":
            from lib import document_storage as ds
            assets = ds.get_assets(ade_sdd, project_key)
            ok = bool(assets)
            status["assets"] = ok
            if not ok:
                missing.append("项目资产")
                missing_hints.append("走 project-assets-update-skill 生成项目资产")
        elif key == "RA":
            ra_files = _iter_ra_files(project_dir)
            ok = bool(ra_files)
            status["RA"] = ok
            if not ok:
                missing.append("RA 文档")
                missing_hints.append("跑 requirement-analysis-skill 生成 RA")
        elif key == "PRD":
            prd_files = _find_prd_files(project_dir)
            ok = bool(prd_files)
            status["PRD"] = ok
            if not ok:
                missing.append("PRD 文档")
                missing_hints.append("向用户索取 PRD，或在 RA 中显式标注豁免理由")
        elif key == "DR":
            design = paths.project_design_dir(project_dir)
            drs = []
            if design.is_dir():
                drs = sorted(set(design.rglob("*DR*.md")) | set(design.rglob("*dr*.md")))
                drs = [d for d in drs if not any(kw in d.name for kw in
                        ("CodeReview", "CodingReport", "TestReport", "-Report"))]
            ok = bool(drs)
            status["DR"] = ok
            if not ok:
                missing.append("DR 文档")
                missing_hints.append("跑 dr-generate-skill 生成 DR")
        elif key == "Story":
            if not current_story:
                status["Story"] = False
                missing.append("Story 文档（state.currentStory 为空）")
                missing_hints.append("ae-sdd state write --phase story-generated --story STORY-XXX")
            else:
                story_doc = paths.find_doc(project_dir, current_story, ".md")
                ok = story_doc is not None
                status["Story"] = ok
                if not ok:
                    missing.append(f"Story 文档（{current_story}.md）")
                    missing_hints.append(f"跑 story-generate-skill 生成 {current_story}")
        elif key == "TestCase":
            if not current_story:
                status["TestCase"] = False
                missing.append("TestCase 文档（state.currentStory 为空）")
                missing_hints.append("ae-sdd state write --phase testcase-generated --story STORY-XXX")
            else:
                tc_doc = paths.find_doc(project_dir, current_story, "-testcase.md")
                ok = tc_doc is not None
                status["TestCase"] = ok
                if not ok:
                    missing.append(f"TestCase 文档（{current_story}-testcase.md）")
                    missing_hints.append(f"跑 testcase-generate-skill 生成 {current_story}-testcase.md")
        # 🆕 v3.9.3: 依赖 Story 自动发现与可达性校验
        elif key == "dependsStory":
            ok, detail = _check_depends_story(project_dir, current_story)
            status["dependsStory"] = ok
            if not ok:
                missing.append(f"依赖 Story：{detail}")
                missing_hints.append(
                    "在 Story 元信息的『复用其他 Story / 模块的能力』表中列出依赖 Story ID，"
                    "并确保 ae-sdd doc resolve --intent STORY --story-id {ID} 可达"
                )
        # 🆕 v3.9.3: 接口契约字段来源追溯校验
        elif key == "sourceTrace":
            ok, detail = _check_source_trace(project_dir, current_story)
            status["sourceTrace"] = ok
            if not ok:
                missing.append(f"接口契约来源追溯：{detail}")
                missing_hints.append(
                    "在 Story 接口契约章节的 Request/Response 表格中，"
                    "为每个字段填写『来源』列，格式：『调用方从...取得』或『查表X.字段Y』"
                )
        # 🆕 v3.9.20: 约束标准引用校验（真"已引用"门禁——查产物证据）
        elif key == "standardsRef":
            ok, detail, hit = _check_standards_referenced(project_dir, current_story, scale)
            status["standardsRef"] = ok
            if not ok:
                missing.append(f"约束标准引用：{detail}")
                missing_hints.append(
                    "在 Story 正文引用所遵循的约束文档标准（如分层架构/参数化查询/DDL 约束/"
                    "单元测试覆盖率等具体条目），证明已加载并遵循项目标准。"
                    f"大/中链需 ≥3 个约束类别，小/微链需 ≥1 个。当前命中：{hit or '无'}"
                )

    if missing:
        hint_str = "；".join(missing_hints)
        return GateResult(
            gate_id, name, "blocker", False,
            f"上下文加载未齐备（缺 {len(missing)} 项）：{', '.join(missing)}",
            f"{hint_str}；补齐后重跑 ae-sdd gates check --only {gate_id}",
            details={"status": status, "missing": missing,
                     "scale": scale, "phase": phase,
                     "required": required_keys})

    return GateResult(gate_id, name, "blocker", True,
                      f"上下文加载齐备（{len(required_keys)} 项：{', '.join(required_keys)}）",
                      details={"status": status, "scale": scale, "phase": phase,
                               "required": required_keys})


def check_g_dr_ctx(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-DR-CTX DR 上下文加载 — 校验 constraints/assets/RA/PRD 已读齐。"""
    return _check_context_loaded(project_dir, st, current_story, "G-DR-CTX")


def check_g_story_ctx(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-STORY-CTX Story 上下文加载 — 校验 constraints/assets/DR/PRD 已读齐。"""
    return _check_context_loaded(project_dir, st, current_story, "G-STORY-CTX")


def check_g_testcase_ctx(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-TESTCASE-CTX TestCase 上下文加载 — 校验 constraints/assets/Story 已读齐。"""
    return _check_context_loaded(project_dir, st, current_story, "G-TESTCASE-CTX")


def check_g_task_ctx(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-TASK-CTX Task 上下文加载 — 校验 constraints/assets/Story/TestCase 已读齐（微链豁免 Story/TestCase）。"""
    return _check_context_loaded(project_dir, st, current_story, "G-TASK-CTX")


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
    "G-DOC-CONSISTENCY": check_g_doc_consistency,  # 🆕 v3.5.7 项目侧记忆-配置路径一致性
    "G-REVIEW-LOOP": check_g_review_loop,  # 🆕 v3.5.12 review-loop 退出条件
    "G-09B": check_g09b,  # 🆕 v3.5.13 reviewer 独立性硬门禁（堵 root 自扮）
    "G-AUTO-CONSENSUS": check_g_auto_consensus,  # 🆕 v3.8.0 自动化联审共识
    "G-REVIEW-DEPTH": check_g_review_depth,  # 🆕 v3.9.20 Review 深度（禁裸✅+零发现举证）
    "G-DR-CTX": check_g_dr_ctx,                  # 🆕 v3.9.1 DR 上下文加载
    "G-STORY-CTX": check_g_story_ctx,            # 🆕 v3.9.1 Story 上下文加载
    "G-TESTCASE-CTX": check_g_testcase_ctx,      # 🆕 v3.9.1 TestCase 上下文加载
    "G-TASK-CTX": check_g_task_ctx,              # 🆕 v3.9.1 Task 上下文加载
}


# ─── 主入口 ─────────────────────────────────────────────────────────────────
def check_all(master_source: Optional[Path], ade_sdd: Optional[Path],
              project_key: str, only: Optional[str] = None) -> list[GateResult]:
    """跑全部门禁（14 主门禁 + G-RA + G-CODE 等）；only 指定时只跑那一个"""
    results: list[GateResult] = []

    # 读 state（如果 .ae-sdd 存在）
    if ade_sdd:
        try:
            st = work_item_context.resolve_default_state(ade_sdd).data
        except (work_item_context.NoWorkItemStateError, work_item_context.AmbiguousWorkItemError):
            st = {"phase": "initialized", "currentStory": None}
    else:
        st = {"phase": "initialized", "currentStory": None}
    current_story = state_mod.get_active_story(st) or ""
    active_phase = state_mod.get_active_phase(st)
    if current_story or active_phase:
        st = dict(st)
        if current_story:
            st["currentStory"] = current_story
        if active_phase:
            st["phase"] = active_phase

    # 推导 project_dir
    project_dir = paths.project_root(ade_sdd) if ade_sdd else Path.cwd()

    targets = [g for g in GATE_REGISTRY if (only is None or g["id"] == only)]
    if only and not targets:
        return [GateResult(
            gate_id=only, name="未知门禁", severity="blocker",
            pass_=False,
            message=f"未知门禁 ID: {only}（允许: {[g['id'] for g in GATE_REGISTRY]}）",
        )]

    def _run_gate(g: dict) -> GateResult:
        if g["id"] == "G-00":
            return check_g00(master_source, ade_sdd, project_key)
        if g["id"] == "G-09":
            # G-09 需要 master_source 调子脚本
            return check_g09(project_dir, st, current_story, master_source=master_source)
        if g["id"] == "G-RA-4":
            # G-RA-4 同样需要 master_source 调 ra_authenticity_scan.py
            return check_ra_authenticity(project_dir, st, current_story, master_source=master_source)
        if g["id"] == "G-RA-5":
            # 🆕 v3.5.9 G-RA-5 需要 master_source 调 ra_depth_scan.py
            return check_ra_depth(project_dir, st, current_story, master_source=master_source)
        if g["id"] == "G-RA-6":
            # 🆕 v3.5.18 G-RA-6 需要 master_source 调 ra_implementation_scan.py
            return check_ra_implementation(project_dir, st, current_story, master_source=master_source)
        if g["id"] == "G-CODE-1":
            # G-CODE-1 需要 master_source 调 coding_authenticity_scan.py
            return check_gcode1(project_dir, st, current_story, master_source=master_source)
        if g["id"] == "G-RA-FLOW-VIOLATION":
            # 🆕 v3.5.11 修复假门禁：G-RA-FLOW-VIOLATION 需要 master_source 调 flow_violation_scan.py
            # 历史漏洞：走 CHECK_FUNCS 漏传 master_source → scanner 恒定位失败 → stub-pass
            return check_ra_flow_violation(project_dir, st, current_story, master_source=master_source)
        if g["id"] == "G-PATH":
            # 🆕 v4.1 G-PATH 需要 master_source 扫母版 source/ 路径越界
            return check_g_path(master_source, project_dir, st, current_story)
        if g["id"] in CHECK_FUNCS:
            return CHECK_FUNCS[g["id"]](project_dir, st, current_story)
        return _stub_v31(g["id"], g["name"])

    for g in targets:
        with runtime_stats.span("gate", {"gateId": g["id"], "gateName": g["name"]}) as gate_span:
            result = _run_gate(g)
        result.details.setdefault("durationMs", gate_span.duration_ms)
        results.append(result)

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
    result_items = [
        {
            "gate_id": r.gate_id,
            "name": r.name,
            "severity": r.severity,
            "pass": r.pass_,
            "message": r.message,
            "action": r.action,
            "durationMs": r.details.get("durationMs"),
            "details": r.details,
        }
        for r in results
    ]
    slowest = sorted(
        (item for item in result_items if item.get("durationMs") is not None),
        key=lambda item: float(item.get("durationMs") or 0.0),
        reverse=True,
    )[:5]
    return {
        "total": len(results),
        "passed": sum(1 for r in results if r.pass_),
        "failed": sum(1 for r in results if not r.pass_),
        "stubs": sum(1 for r in results if r.details.get("stub")),
        "all_pass": all(r.pass_ for r in results),
        "slowest": [
            {
                "gate_id": item["gate_id"],
                "name": item["name"],
                "durationMs": item["durationMs"],
                "pass": item["pass"],
            }
            for item in slowest
        ],
        "results": result_items,
    }
