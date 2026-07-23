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

import json
import re
import sys
import tempfile
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable, Optional

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from lib import document_storage, paths, runtime_exec, runtime_stats, state as state_mod, work_item_context  # noqa: E402
from scripts.ra_scan_scope import classify_formal_ra, resolve_ra_scan_scope  # noqa: E402


_GATE_SUBPROCESS_LIMIT_SECONDS = 60
_GIT_INSPECTION_LIMIT_SECONDS = 10


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


# 门禁元信息（GATE_REGISTRY 实际 36 条；v3.10.0 砍 Task phase 后 G-04/G-05/G-06/G-TASK-CTX 仍注册但不再在 PHASE_ENTRY_GATES 中触发）
# 🆕 v3.10.3 hint 字段迁入本注册表（单一权威源）：每个 gate 自带 scope/pass/fail 三元组，
# 编译器 render_gates_compact 直接从 gate["hint"] 读取，不再维护独立的 GATE_HINTS 字典。
GATE_REGISTRY: list[dict] = [
    {"id": "G-00", "name": "项目资产完整性",       "severity": "blocker",
     "hint": {"scope": "entry", "pass": "project assets exist and 7-layer index is complete", "fail": "BLOCK -> project-assets-update"}},
    {"id": "G-01", "name": "DR 文档存在",          "severity": "blocker",
     "hint": {"scope": "before Story/Task generation", "pass": "DR document exists (large route)", "fail": "BLOCK"}},
    {"id": "G-02", "name": "Story 文档存在",       "severity": "blocker",
     "hint": {"scope": "before TestCase/Coding generation", "pass": "Story document exists", "fail": "BLOCK"}},
    {"id": "G-03", "name": "Story Review 通过",    "severity": "blocker",
     "hint": {"scope": "before TestCase generation", "pass": "Story review loop exited normally", "fail": "BLOCK -> re-review"}},
    {"id": "G-04", "name": "TestCase 文档存在",    "severity": "blocker",
     "hint": {"scope": "before Coding generation", "pass": "TestCase document exists", "fail": "BLOCK"}},
    {"id": "G-05", "name": "Task 文档存在",        "severity": "blocker",
     "hint": {"scope": "before Coding execute (legacy)", "pass": "Task document exists (v3.10 skeleton merged into coding-process)", "fail": "BLOCK"}},
    {"id": "G-06", "name": "Task Review 通过",     "severity": "blocker",
     "hint": {"scope": "before Coding execute (legacy)", "pass": "Task review passed (v3.10 merged into coding-process)", "fail": "BLOCK"}},
    {"id": "G-07", "name": "CodingPlan 存在",      "severity": "blocker",
     "hint": {"scope": "before coding execute", "pass": "CodingPlan document exists", "fail": "BLOCK"}},
    {"id": "G-08", "name": "CodingPlan 14 门禁通过", "severity": "blocker",
     "hint": {"scope": "before coding execute", "pass": "CodingPlan 14 gates are present", "fail": "BLOCK"}},
    {"id": "G-HTTP-1", "name": "HTTP 场景推导有效", "severity": "blocker",
     "hint": {"scope": "before coding execute", "pass": "HTTP AC has a derived, repeatable, independently observed scenario manifest", "fail": "BLOCK"}},
    {"id": "G-09", "name": "测试真实性扫描通过",   "severity": "blocker",
     "hint": {"scope": "test review", "pass": "test authenticity scanner passes", "fail": "BLOCK"}},
    {"id": "G-10", "name": "测试报告存在",         "severity": "blocker",
     "hint": {"scope": "after test run", "pass": "test report document exists", "fail": "BLOCK -> run test"}},
    {"id": "G-11", "name": "Coding 报告存在",      "severity": "blocker",
     "hint": {"scope": "after coding", "pass": "coding report document exists", "fail": "BLOCK"}},
    {"id": "G-12", "name": "CodeReview 报告存在",  "severity": "blocker",
     "hint": {"scope": "after code review", "pass": "CodeReview report document exists", "fail": "BLOCK"}},
    {"id": "G-13", "name": "全链路对称性核查通过", "severity": "blocker",
     "hint": {"scope": "delivery check", "pass": "full-chain symmetry check passes", "fail": "BLOCK -> fix gaps"}},
    # 🆕 v3.4.0 中段门禁（对标建议书1/2/4 - 补齐"两头强中间空"的中段强制力）
    # G-14 CodingPlan-Story 一致性：Plan 涉及接口/DO/AC 与 Story 可对应（建议书4 G-08-15）
    {"id": "G-14", "name": "CodingPlan-Story 一致性", "severity": "blocker",
     "hint": {"scope": "before coding execute", "pass": "CodingPlan references Story and aligns with AC", "fail": "BLOCK"}},
    # G-CODEPLAN-SRC CodingPlan 源码核对：新增/修改类建模范式须附已读源码标记（建议书1）
    {"id": "G-CODEPLAN-SRC", "name": "CodingPlan 源码核对", "severity": "blocker",
     "hint": {"scope": "before coding execute", "pass": "CodingPlan class skeleton has source-read evidence", "fail": "BLOCK"}},
    # G-DOC-STORAGE 文档落地存放合规：产物路径/命名须经 resolve_path 推导（建议书2）
    {"id": "G-DOC-STORAGE", "name": "文档落地存放合规", "severity": "blocker",
     "hint": {"scope": "before doc write", "pass": "path/name resolved by document-storage", "fail": "BLOCK"}},
    # 🆕 v4.1 G-PATH 路径越界检测：母版 source/ 下 SKILL/template 文档不得硬编码产出路径，
    # 须声明调用 document-storage（防自身路径规则漂移，与 plugin_content_scan PC-009/010 分层防护）
    {"id": "G-PATH", "name": "路径越界检测", "severity": "blocker",
     "hint": {"scope": "build/update check", "pass": "source docs do not hardcode output paths", "fail": "BLOCK"}},
    # 🆕 v3.2 G-RA 需求分析准入门卫（对标 SKILL.md §🛡️ G-RA）- 把 RA 16 道闸
    # 的核心条款从"纸面规则"变成"可执行门禁"，与 Coding G-08/G-09 对等。
    {"id": "G-RA-1", "name": "RA 文档存在",          "severity": "blocker",
     "hint": {"scope": "before DR/Story/Task generation", "pass": "RA document exists or route is exempt", "fail": "BLOCK"}},
    {"id": "G-RA-2", "name": "RA 8 维度完整",        "severity": "blocker",
     "hint": {"scope": "before DR/Story/Task generation", "pass": "RA dimensions and RAModel are complete", "fail": "BLOCK"}},
    {"id": "G-RA-3", "name": "RA 衍生章节完整",      "severity": "blocker",
     "hint": {"scope": "before DR/Story/Task generation", "pass": "RA derivative sections are complete", "fail": "BLOCK"}},
    {"id": "G-RA-4", "name": "RA 真实性扫描通过",    "severity": "blocker",
     "hint": {"scope": "before DR/Story/Task generation", "pass": "RA authenticity scanner passes", "fail": "BLOCK"}},
    # 🆕 2026-06-27 RA 流程违规审计（建议书 §3.4）- 扫 RA 文档是否走完 RAModel 12 维 +
    # 8 维度 + 5 问自检 + 缺口管理 + 规模裁定 + RA-G01~16 闸判定，堵"AI 跳过 RA 完整流程直接出 RA 文档"
    {"id": "G-RA-FLOW-VIOLATION", "name": "RA 流程违规审计", "severity": "blocker",
     "hint": {"scope": "before downstream generation", "pass": "RA flow violation scanner passes", "fail": "BLOCK"}},
    # 🆕 v3.5.9 RA 机械派生深度通过（防「形式通过、内容空转」）
    # 与 G-RA-3（章节锚点存在）/G-RA-4（无 fabricate/vague）/G-RA-FLOW-VIOLATION（流程完整性）
    # 正交：本门禁验证 E.5/G.5/H.6/H.5 规定的「每行 R->R′->AC 机械追问」是否真做了。
    # 5 条规则：D1 §6.5 主规则机械派生 + D2 R′->AC 链接 + D3 §8.6 覆盖率真实重算
    #        + D4 §9-ter 五问机械覆盖 + D5 §9-bis 业务模式六选一
    {"id": "G-RA-5", "name": "RA 机械派生深度通过",  "severity": "blocker",
     "hint": {"scope": "before DR/Story/Task generation", "pass": "RA mechanical derivation scanner passes", "fail": "BLOCK"}},
    # 🆕 v3.5.18 RA 实现视角完整性：挖出数据源、数据流、定义/不变量、复用证据、
    # 高成本/难实现设计反驳、开发者疑问答复、DR 交接包，防 RA 只能写产品话术而无法支撑实现。
    {"id": "G-RA-6", "name": "RA 实现视角完整性通过",  "severity": "blocker",
     "hint": {"scope": "before DR/Story/Task generation", "pass": "RA implementation-view scanner passes", "fail": "BLOCK"}},
    {"id": "G-CODE-1", "name": "Coding 真实性扫描通过", "severity": "blocker",
     "hint": {"scope": "coding/code review", "pass": "coding authenticity scanner passes", "fail": "BLOCK"}},
    # 🆕 v3.5.7 项目侧记忆-配置路径一致性：项目 AGENTS.md/.harness/memory/MEMORY.md 等
    # "文档工作区"表述须与 .ae-sdd/config.yaml 的 docWorkspacePath 一致，防旧记忆劫持新配置
    # （实测案例：life 项目 MEMORY 写 D:\Item\doc 与 config 写 D:\Item\life 冲突，RA 落错位置）
    {"id": "G-DOC-CONSISTENCY", "name": "项目侧记忆-配置路径一致性", "severity": "blocker",
     "hint": {"scope": "entry/doc workspace check", "pass": "project memory path agrees with config", "fail": "BLOCK"}},
    # 🆕 v3.5.12 review-loop 退出条件门禁：review 节点（story/dr/task/code review）切相前，
    # 校验 reviewLoop.exitReason 满足协议（normal 需 dryCounter≥2 / escalate 已升级用户）。
    # 治 P0-1/4：堵 root agent 单轮就自称"连续2轮无新增"退出，无机械反驳。
    # 注：本门禁依赖 root agent 跑过 `ae-sdd review-loop collect`（无 reviewLoop 字段时降级 skip）
    {"id": "G-REVIEW-LOOP", "name": "review-loop 退出条件通过", "severity": "blocker",
     "hint": {"scope": "review phase transition", "pass": "review-loop exit condition is satisfied", "fail": "BLOCK"}},
    # 🆕 v3.5.13 G-09B reviewer 独立性硬门禁：review 节点切相时机械派生 Tier，
    # 校验 state.activeAgents 有 ≥Tier 个 sessionId≠root 的 reviewer。
    # 独立于 review-loop CLI--root 不调 collect 也会跑（堵"root 总派给自己"）。
    # Tier 1（微/小任务 + 无关键决策）豁免；Tier 2/3 无豁免。
    {"id": "G-09B", "name": "reviewer 独立性通过（多 reviewer 机械强制）", "severity": "blocker",
     "hint": {"scope": "review phase transition", "pass": "reviewer independence requirement passes", "fail": "BLOCK"}},
    # 🆕 v3.9.20 G-REVIEW-DEPTH Review 深度门禁：禁裸✅ + 零发现举证。
    # 治根因：Review 无深度门禁，reviewer 可把所有维度标"无缺陷"而所有门禁照过
    # （code-review-skill L21-24 坦承"当前为软门禁 report-only"）。本门禁查报告内容证据。
    {"id": "G-REVIEW-DEPTH", "name": "Review 深度（禁裸✅ + 零发现举证）", "severity": "blocker",
     "hint": {"scope": "review phase transition", "pass": "review report has depth evidence (no bare ✅, zero-finding justified)", "fail": "BLOCK -> add evidence or findings"}},
    # 🆕 v3.8.0 G-AUTO-CONSENSUS 自动化联审共识门禁：自动化模式下审核点切相前
    # 校验 state.reviewConsensus[point].passed=true + reviewer 独立性（复用 G-09B 逻辑）。
    # 非自动化模式 / 审核点不在白名单 -> skipped（回退人工审核）。
    # 注：本门禁需读 config.yaml 判自动化模式，走 check_all 特判传 master_source。
    {"id": "G-AUTO-CONSENSUS", "name": "自动化联审共识通过", "severity": "blocker",
     "hint": {"scope": "automation mode review transition", "pass": "state.reviewConsensus[point].passed=true + reviewer independence", "fail": "BLOCK (non-automation or off-whitelist -> skipped)"}},
    # 🆕 v3.9.1 上下文加载准入门禁（注册表模式）- 对齐 RA/Coding 的「prose+CLI+门禁」三合一，
    # 把 DR/Story/TestCase/Task 四组的「第零步准入检查」从 prose 变成机械阻断。
    # 治「AI 不读 PRD/DR/项目资产/约束就过门禁切相」的真空带。
    # 注册表 CONTEXT_GATE_REGISTRY 定义各 gate 的 scale 适用范围 + required 上下文清单。
    {"id": "G-DR-CTX", "name": "DR 上下文加载", "severity": "blocker",
     "hint": {"scope": "before DR generation", "pass": "required DR contexts loaded (PRD/assets/constraints/standards)", "fail": "BLOCK -> load CONTEXT_GATE_REGISTRY[G-DR-CTX].required"}},
    {"id": "G-STORY-CTX", "name": "Story 上下文加载", "severity": "blocker",
     "hint": {"scope": "before Story generation", "pass": "required Story contexts loaded (constraints/assets/DR/PRD/dependsStory/sourceTrace/standardsRef/outputBoundary)", "fail": "BLOCK -> load CONTEXT_GATE_REGISTRY[G-STORY-CTX].required"}},
    {"id": "G-TESTCASE-CTX", "name": "TestCase 上下文加载", "severity": "blocker",
     "hint": {"scope": "before TestCase generation", "pass": "required TestCase contexts loaded", "fail": "BLOCK -> load CONTEXT_GATE_REGISTRY[G-TESTCASE-CTX].required"}},
    {"id": "G-TASK-CTX", "name": "Task 上下文加载", "severity": "blocker",
     "hint": {"scope": "before Task generation (legacy)", "pass": "required Task contexts loaded (v3.10 merged into coding-process)", "fail": "BLOCK -> load CONTEXT_GATE_REGISTRY[G-TASK-CTX].required"}},
]

# Story Review 之后允许的 phase
# 🆕 v3.10.1 子系列合并：story-generated = generate+review loop 已完成（2轮无新缺陷退出）
# 故 story-generated 本身即代表 Story Review 已通过，加入此集合。
PHASE_PAST_STORY_REVIEW = {
    "story-generated",  # 🆕 v3.10.1 合并后：进入 story-generated = review 已过
    "story-reviewed",   # 🟡 兼容旧 state（已拆分的 phase）
    "testcase-generated", "testcase-reviewed",  # 🟡 兼容旧 state
    "coding-process",  # 🆕 v3.10.0 砍 Task：testcase 之后直入 coding-process
    "coding", "test-running", "code-reviewed", "completed",
}

# Task Review 之后允许的 phase（🟡 v3.10.0：Task phase 已从主流程移除，
# 此集合仅保留兼容旧 state；G-06 不再在 PHASE_ENTRY_GATES 中触发）
PHASE_PAST_TASK_REVIEW = {
    "task-reviewed", "coding-process", "coding", "test-running", "code-reviewed", "completed",
}

# CodingPlan 必含章节（按 coding-skill §5 CodePlan 7 章节，被 coding-process §A 调用）
CODINGPLAN_REQUIRED_SECTIONS = [
    "文件顺序", "类骨架", "数据", "Mapper SQL", "测试对应", "验证点", "调试回滚",
]

MICRO_CODINGPLAN_DIMENSIONS: dict[str, tuple[str, ...]] = {
    "scope": ("change scope", "implementation scope", "planned files", "变更范围", "改动范围", "文件清单", "文件级实现顺序"),
    "implementation": ("implementation sequence", "implementation steps", "执行顺序", "实现顺序", "实施步骤", "改动清单"),
    "risk_rollback": ("risks and rollback", "risk", "rollback", "风险", "回滚", "停止条件", "升级条件"),
    "verification": ("verification", "test plan", "tests", "验证", "测试"),
}


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


# DR 文档排除关键词（非 DR 的报告类产物，G-01/G-13/G-DR-CTX 共用）
_DR_EXCLUDE_KEYWORDS = ("CodeReview", "CodingReport", "TestReport", "-Report")


def _iter_dr_docs(design_dir: Path) -> list[Path]:
    """枚举 design/ 下的 DR 文档（rglob 递归子目录 + 排除报告类产物）。

    G-01 / G-13 / G-DR-CTX 三处枚举 DR 的共用 helper（DRY）。
    v3.10.5 修复 G-13 漏 rglob：原 design.glob('*DR*.md') 只看根一层，
    漏判 design/story/be/DR-001.md 等子目录产物，与 G-01 行为不一致。
    """
    if not design_dir.is_dir():
        return []
    scan_root = design_dir
    if design_dir.name.lower() == "ae-sdd-doc":
        scan_root = design_dir / "DR"
        if not scan_root.is_dir():
            return []
    drs = sorted(set(scan_root.rglob("*DR*.md")) | set(scan_root.rglob("*dr*.md")))
    return [d for d in drs if not any(kw in d.name for kw in _DR_EXCLUDE_KEYWORDS)]


def _find_task_docs(project_dir: Path, current_story: str) -> list[Path]:
    """枚举本 Story 的 Task 文档（G-05 / G-13 共用，DRY）。

    v3.10.0 砍 Task 后主产物为 {story}.md（document_storage TASK 模板产出
    Task/{story_id}/{story_id}.md）；旧布局为 {story}-task-*.md。
    优先主产物，回退旧布局，与 check_g05 行为一致。
    """
    primary = paths.list_docs(project_dir, current_story, ".md")
    if primary:
        return primary
    return paths.list_docs(project_dir, current_story, "-task-*.md")


def _g13_design_root(project_dir: Path) -> Path:
    """Return the trace document root, preferring legacy design when present."""
    legacy = paths.project_design_dir(project_dir)
    if legacy.is_dir():
        return legacy
    for root in paths.doc_search_roots(project_dir):
        canonical = root / "ae-sdd-doc"
        if canonical.is_dir():
            return canonical
    return legacy


# ─── G-01 ───────────────────────────────────────────────────────────────────
def check_g01(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-01 DR 文档存在"""
    design = _g13_design_root(project_dir)
    if not design.is_dir():
        return GateResult("G-01", "DR 文档存在", "blocker", False,
                          f"DR 文档目录不存在: {design}",
                          "跑 dr-generate-skill 生成 DR 文档")

    # 🆕 v3.5.10 Gap-004：原用 design.glob("*DR*.md") 只看根一层，不递归子目录，
    # 漏判 design/story/be/STORY-001-BE-CodingReport.md 等子目录产物。
    # 改为 rglob + 排除 -CodeReview / -Report 等非 DR 文档，避免误判。
    drs = _iter_dr_docs(design)
    if not drs:
        return GateResult("G-01", "DR 文档存在", "blocker", False,
                          f"{design} 无 DR 文档（rglob *DR*.md / *dr*.md，已排除报告类）",
                          "跑 dr-generate-skill 生成 DR 文档；"
                          "或由主流程监管器按 classify.spec_strategy 自动派 dr-generate 子流程生成")
    return GateResult("G-01", "DR 文档存在", "blocker", True,
                      f"找到 {len(drs)} 个 DR 文档",
                      details={"files": [str(d.relative_to(project_dir)) for d in drs]})


# ─── Story document resolution shared by G-02 / G-14 ───────────────────────
def _resolve_story_doc(project_dir: Path, st: dict,
                       current_story: str) -> tuple[Optional[Path], dict]:
    binding = state_mod.get_story_document_binding(st, current_story)
    details = {
        "storyId": current_story,
        "storyName": binding.get("storyName") or "",
        "storyDocSource": "none",
        "storyDocCandidates": [],
        "storyDocRejected": [],
    }
    try:
        resolution = document_storage.resolve_story_document(
            project_dir,
            story_id=current_story,
            story_name=binding.get("storyName") or "",
            bound_path=binding.get("docPath") or None,
        )
    except document_storage.StoryDocumentAmbiguousError as exc:
        return None, {
            **details,
            "errorCode": exc.code,
            "storyDocCandidates": list(exc.candidates),
        }
    except document_storage.DocStorageError as exc:
        return None, {**details, "errorCode": exc.code, "error": str(exc)}

    resolved_details = {
        **details,
        "storyName": resolution.story_name,
        "storyDocSource": resolution.source,
        "storyDocCandidates": [str(path) for path in resolution.candidates],
        "storyDocRejected": list(resolution.rejected),
    }
    if resolution.path is None and resolution.rejected:
        resolved_details["errorCode"] = resolution.rejected[0]["code"]
    return resolution.path, resolved_details


def _story_binding_action(st: dict, current_story: str) -> str:
    work_item = (_work_item_from_state(st) or "<WorkItem>")
    return (
        "运行 ae-sdd state bind-story-doc "
        f"--work-item {work_item} --story {current_story} "
        "--story-name <正式Story文件名>；"
        "或由主流程监管器按 classify.spec_strategy 自动派 story-generate 子流程生成"
    )


# ─── G-02 ───────────────────────────────────────────────────────────────────
def check_g02(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-02 Story 文档存在"""
    if not current_story:
        return GateResult("G-02", "Story 文档存在", "blocker", False,
                          "state.currentStory 为空",
                          "跑 ae-sdd state write --phase story-generated --story STORY-XXX")

    story, resolution = _resolve_story_doc(project_dir, st, current_story)
    if story is None:
        error_code = resolution.get("errorCode")
        if error_code:
            message = f"Story 文档绑定无效: {error_code}"
        else:
            message = f"Story 文档不存在或未绑定: {current_story}"
        return GateResult("G-02", "Story 文档存在", "blocker", False,
                          message,
                          _story_binding_action(st, current_story),
                          details={
                              **resolution,
                              "expected": str(paths.project_design_dir(project_dir) / f"{current_story}.md"),
                          })
    return GateResult("G-02", "Story 文档存在", "blocker", True,
                      f"找到 {story.name}",
                      details={**resolution, "file": str(story)})


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
    """G-04 验证设计存在：优先 Story 内嵌矩阵，兼容独立 TestCase。"""
    if not current_story:
        return GateResult("G-04", "TestCase 文档存在", "blocker", False,
                          "state.currentStory 为空")

    tc = paths.find_doc(project_dir, current_story, "-testcase.md")
    if tc is None:
        story_doc, resolution = _resolve_story_doc(project_dir, st, current_story)
        if story_doc is not None:
            content = story_doc.read_text(encoding="utf-8", errors="replace")
            has_matrix = (
                "验证矩阵" in content
                or ("验收标准" in content and re.search(r"\bTC[-_]?\d+\b|\bAC[-_]?\d+\b", content))
            )
            if has_matrix:
                return GateResult(
                    "G-04", "验证设计存在", "blocker", True,
                    "Story 已内嵌 AC/验证矩阵（不需要独立 TestCase）",
                    details={**resolution, "file": str(story_doc), "mode": "story-embedded"},
                )
        return GateResult(
            "G-04", "验证设计存在", "blocker", False,
            "Story 无验证矩阵，且独立 TestCase 不存在",
            "在 Story 中补 AC/验证矩阵；仅复杂矩阵才显式生成 TESTCASE",
            details={"mode": "missing"},
        )
    return GateResult("G-04", "TestCase 文档存在", "blocker", True,
                      f"找到 {tc.name}",
                      details={"file": str(tc)})


# ─── G-05 ───────────────────────────────────────────────────────────────────
def check_g05(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-05 Task 文档存在

    🆕 v3.10.4 修复 glob 与 TASK 模板不一致：
    document_storage._PATH_TEMPLATES["TASK"] 产出 ``{docId}.md``（doc_id 缺省 =
    story_id），即 ``STORY-001.md``。原 glob ``-task-*.md`` 拼成
    ``STORY-001-task-*.md`` 永远匹配不上，导致 G-05 假阴性（Pbl.md 问题1）。
    现优先检查与模板对齐的 ``{story_id}.md``，同时保留对旧布局
    ``{story_id}-task-*.md`` 的兼容（旧 task/ 目录或历史归档）。
    """
    if st.get("processPolicy") == "compact":
        return GateResult("G-05", "Task 文档存在", "blocker", True,
                          "compact 流程不生成 Task 文档（兼容门禁跳过）",
                          details={"skipped": True, "processPolicy": "compact"})

    if not current_story:
        return GateResult("G-05", "Task 文档存在", "blocker", False,
                          "state.currentStory 为空")

    # 主检查：与 TASK 模板产出对齐的 {story_id}.md（v3.10 砍 Task 后的实际产物）
    primary = paths.list_docs(project_dir, current_story, ".md")
    if primary:
        return GateResult("G-05", "Task 文档存在", "blocker", True,
                          f"找到 {len(primary)} 个 Task 文档",
                          details={"files": [t.name for t in primary]})

    # 兼容旧布局：{story_id}-task-*.md（历史 task/ 目录或 v3.7 前归档）
    legacy = paths.list_docs(project_dir, current_story, "-task-*.md")
    if legacy:
        return GateResult("G-05", "Task 文档存在", "blocker", True,
                          f"找到 {len(legacy)} 个 Task 文档（旧布局）",
                          details={"files": [t.name for t in legacy]})

    return GateResult("G-05", "Task 文档存在", "blocker", False,
                      f"task/ 目录无 {current_story}.md 或 {current_story}-task-*.md",
                      f"跑 task-generate-skill 生成 Task 文档",
                      details={"task_dir": str(paths.project_task_dir(project_dir))})


# ─── G-06 ───────────────────────────────────────────────────────────────────
def check_g06(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-06 Task Review 通过"""
    if st.get("processPolicy") == "compact":
        return GateResult("G-06", "Task Review 通过", "blocker", True,
                          "compact 流程无 Task Review 阶段（兼容门禁跳过）",
                          details={"skipped": True, "processPolicy": "compact"})
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
    plan = st.get("executionPlan") if isinstance(st.get("executionPlan"), dict) else {}
    if plan.get("goal") or plan.get("changedPaths") or plan.get("verification"):
        missing = [name for name, value in (
            ("goal", plan.get("goal")),
            ("changedPaths", plan.get("changedPaths")),
            ("verification", plan.get("verification")),
        ) if not value]
        if missing:
            return GateResult(
                "G-07", "ExecutionPlan 存在", "blocker", False,
                f"state.executionPlan 缺必填字段: {missing}",
                "补充 goal、changedPaths、verification 后重新确认",
                details={"profile": "compact", "missing": missing},
            )
        return GateResult(
            "G-07", "ExecutionPlan 存在", "blocker", True,
            "结构化 executionPlan 已就绪",
            details={"profile": "compact", "approved": bool(plan.get("approved")),
                     "changedPaths": list(plan.get("changedPaths") or [])},
        )
    cp, resolution = _resolve_codingplan_doc(project_dir, st, current_story)
    if resolution.get("errorCode"):
        return GateResult("G-07", "CodingPlan 存在", "blocker", False,
                          "CodingPlan legacy Story 候选不唯一，拒绝猜测",
                          "显式传入当前 Work Item 或清理重复 CodingPlan",
                          details=resolution)
    if cp is None:
        return GateResult("G-07", "CodingPlan 存在", "blocker", False,
                          "CodingPlan 文档不存在",
                          "跑 CodingPlan 生成（CodingProcess §A 调 coding-skill §5）",
                          details=resolution)

    content = cp.read_text(encoding="utf-8")
    if st.get("scale") == "微":
        if not content.strip():
            return GateResult("G-07", "CodingPlan 存在", "blocker", False,
                              "微任务 CodingPlan 为空",
                              "补充变更范围、实现顺序、风险/回滚与验证计划",
                              details={**resolution, "profile": "micro"})
        return GateResult("G-07", "CodingPlan 存在", "blocker", True,
                          f"{cp.name} 微任务 CodingPlan 已存在（结构质量由 G-08 校验）",
                          details={**resolution, "profile": "micro", "file": str(cp)})

    missing = [s for s in CODINGPLAN_REQUIRED_SECTIONS if s not in content]
    if missing:
        return GateResult("G-07", "CodingPlan 存在", "blocker", False,
                          f"CodingPlan 缺章节: {missing}",
                          "补全 7 章节（coding-skill §5 CodePlan 7 章节）",
                          details={"missing_sections": missing})
    return GateResult("G-07", "CodingPlan 存在", "blocker", True,
                      f"{cp.name} 7 章节齐全",
                      details={**resolution, "profile": "full", "file": str(cp)})


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
                     legacy_suffixes: list[str],
                     work_item: str = "") -> Optional[Path]:
    return _resolve_report_doc(
        project_dir,
        current_story,
        category=category,
        patterns=patterns,
        legacy_suffixes=legacy_suffixes,
        work_item=work_item,
    ).path


def _work_item_from_state(st: dict) -> str:
    return str(
        st.get("_resolvedWorkItem")
        or st.get("stateMachineName")
        or st.get("currentWorkItem")
        or st.get("workItemKey")
        or ""
    )


def _resolve_report_doc(project_dir: Path, current_story: str, *,
                        category: str, patterns: list[str],
                        legacy_suffixes: list[str], work_item: str = ""):
    suffixes = list(legacy_suffixes)
    for pattern in patterns:
        if current_story and pattern.startswith(current_story):
            suffix = pattern[len(current_story):]
            if suffix not in suffixes:
                suffixes.append(suffix)
    return document_storage.resolve_scoped_artifact(
        project_dir,
        category=category,
        work_item_id=work_item or current_story,
        story_id=current_story,
        suffixes=suffixes,
    )


def _resolve_codingplan_doc(project_dir: Path, st: dict,
                            current_story: str) -> tuple[Optional[Path], dict]:
    """Resolve a CodingPlan by Work Item first, with unique Story fallback."""
    work_item = _work_item_from_state(st) or current_story
    details = {"workItem": work_item, "scopeSource": "none"}
    if not work_item:
        return None, details
    try:
        resolution = document_storage.resolve_scoped_artifact(
            project_dir,
            category="Coding",
            work_item_id=work_item,
            story_id=current_story,
            suffixes=["-CodingPlan.md"],
        )
    except document_storage.ScopeAmbiguousError as exc:
        return None, {
            **details,
            "errorCode": exc.code,
            "scopeSource": "legacy-story",
            "candidates": [str(path) for path in exc.candidates],
        }
    return resolution.path, {
        **details,
        "scopeSource": resolution.scope_source,
        "candidates": [str(path) for path in resolution.candidates],
    }


def _check_report(project_dir: Path, st: dict, current_story: str, *,
                 gate_id: str, name: str, category: str,
                 patterns: list[str], legacy_suffixes: list[str],
                 expected_hint: str, action: str) -> GateResult:
    """报告类门禁的通用检查逻辑"""
    if not current_story:
        return GateResult(gate_id, name, "blocker", False,
                          "state.currentStory 为空")

    work_item = _work_item_from_state(st) or current_story
    try:
        resolution = _resolve_report_doc(
            project_dir, current_story,
            category=category,
            patterns=patterns,
            legacy_suffixes=legacy_suffixes,
            work_item=work_item,
        )
    except document_storage.ScopeAmbiguousError as exc:
        return GateResult(
            gate_id, name, "blocker", False,
            "legacy Story 报告候选不唯一，拒绝猜测",
            "显式生成当前 Work Item 报告或清理重复 legacy 候选",
            details={
                "errorCode": exc.code,
                "scopeSource": "legacy-story",
                "candidates": [str(path) for path in exc.candidates],
            },
        )
    doc = resolution.path
    if doc is None:
        return GateResult(gate_id, name, "blocker", False,
                          f"报告文档不存在: {expected_hint}",
                          action,
                          details={"expected": expected_hint,
                                   "workItem": work_item,
                                   "scopeSource": resolution.scope_source})
    return GateResult(gate_id, name, "blocker", True,
                      f"找到 {doc.name}",
                      details={"file": str(doc),
                               "workItem": work_item,
                               "scopeSource": resolution.scope_source})


def check_g10(project_dir: Path, st: dict, current_story: str) -> GateResult:
    if current_story:
        from lib import evidence
        manifest = evidence.manifest_path(project_dir, current_story)
        if manifest.is_file():
            return GateResult(
                "G-10", "测试证据存在", "blocker", True,
                "evidence manifest 已存在（不生成 TestReport）",
                details={"manifest": str(manifest), "processArtifactPolicy": "evidence-only"},
            )
    return _check_report(
        project_dir, st, current_story,
        gate_id="G-10", name="测试证据存在（兼容旧 TestReport）",
        category="Test", patterns=[f"{current_story}-Report.md", f"{current_story}-Report-v*-r*.md"],
        legacy_suffixes=["-Report.md"], expected_hint=f"evidence manifest for {current_story}",
        action="记录真实测试 evidence；不要生成 TestReport",
    )


def check_g11(project_dir: Path, st: dict, current_story: str) -> GateResult:
    plan = st.get("executionPlan") if isinstance(st.get("executionPlan"), dict) else {}
    if plan.get("approved") and plan.get("changedPaths"):
        return GateResult(
            "G-11", "Coding 交付证据存在", "blocker", True,
            "approved executionPlan + changedPaths 已记录（不生成 CodingReport）",
            details={"changedPaths": list(plan.get("changedPaths") or []),
                     "processArtifactPolicy": "git-diff+evidence"},
        )
    return _check_report(
        project_dir, st, current_story,
        gate_id="G-11", name="Coding 交付证据存在（兼容旧 CodingReport）",
        category="Coding",
        patterns=[f"{current_story}-CodingReport.md", f"{current_story}-CodingReport-v*-r*.md",
                  f"{current_story}-Coding-Report-v*-r*.md"],
        legacy_suffixes=["-CodingReport.md", "-Coding-Report.md"],
        expected_hint="approved state.executionPlan.changedPaths",
        action="记录并批准 executionPlan；不要生成 CodingReport",
    )


def check_g12(project_dir: Path, st: dict, current_story: str) -> GateResult:
    review = st.get("review") if isinstance(st.get("review"), dict) else {}
    status = review.get("status")
    findings = review.get("findings") if isinstance(review.get("findings"), list) else []
    if status == "passed" and not findings:
        return GateResult(
            "G-12", "CodeReview 结论存在", "blocker", True,
            "review.status=passed，findings 为空（不生成 CodeReview 报告）",
            details={"review": review, "processArtifactPolicy": "findings-only"},
        )
    if status == "changes_required" and findings:
        return GateResult(
            "G-12", "CodeReview 结论存在", "blocker", False,
            f"Review 有 {len(findings)} 个 findings 待修复",
            "修复 findings 后重新记录 review.status",
            details={"review": review, "processArtifactPolicy": "findings-only"},
        )
    return _check_report(
        project_dir, st, current_story,
        gate_id="G-12", name="CodeReview 结论存在（兼容旧报告）",
        category="CR", patterns=[f"{current_story}-CodeReview.md", f"{current_story}-CodeReview-v*-r*.md"],
        legacy_suffixes=["-CodeReview.md"], expected_hint="state.review.status/findings",
        action="记录 review.status/findings；不要生成 CodeReview 报告",
    )


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
    compact_review = st.get("review") if isinstance(st.get("review"), dict) else {}
    if compact_review.get("status") in {"passed", "changes_required"}:
        findings = compact_review.get("findings") or []
        invalid = [item for item in findings if not isinstance(item, dict) or not item.get("severity")]
        if invalid:
            return GateResult(
                gate_id, name, "blocker", False,
                "review.findings 缺 severity 等结构化证据",
                "为每个 finding 补 severity/file/line/problem/requiredAction",
                details={"profile": "findings-only", "invalidCount": len(invalid)},
            )
        return GateResult(
            gate_id, name, "blocker", True,
            "结构化 Review findings/status 已满足深度门禁（不读取 Markdown 报告）",
            details={"profile": "findings-only", "status": compact_review.get("status"),
                     "findings": len(findings), "reviewedPaths": compact_review.get("reviewedPaths", [])},
        )
    work_item = _work_item_from_state(st) or current_story
    try:
        resolution = _resolve_report_doc(
            project_dir, current_story,
            category="CR",
            patterns=[f"{current_story}-CodeReview.md",
                      f"{current_story}-CodeReview-v*-r*.md"],
            legacy_suffixes=["-CodeReview.md"],
            work_item=work_item,
        )
    except document_storage.ScopeAmbiguousError as exc:
        return GateResult(
            gate_id, name, "blocker", False,
            "legacy Story CodeReview 候选不唯一，拒绝执行深度判定",
            "显式生成当前 Work Item CodeReview 报告",
            details={"errorCode": exc.code,
                     "scopeSource": "legacy-story",
                     "candidates": [str(path) for path in exc.candidates]},
        )
    doc = resolution.path
    if doc is None:
        return GateResult(gate_id, name, "blocker", True,
                          "CodeReview 报告不存在（skip，由 G-12 兜底报告存在性）",
                          details={"skipped": True,
                                   "scopeSource": resolution.scope_source})
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


def _http_verification_items(plan: dict) -> list[dict]:
    verification = plan.get("verification") if isinstance(plan, dict) else None
    if not isinstance(verification, list):
        return []
    return [
        item for item in verification
        if isinstance(item, dict)
        and str(item.get("boundary") or "").strip().casefold() == "http"
    ]


def _http_verification_ac_ids(plan: dict) -> list[str]:
    ac_ids = []
    for item in _http_verification_items(plan):
        ac_id = str(item.get("acId") or item.get("ac") or "").strip()
        if ac_id:
            ac_ids.append(ac_id)
    return sorted(set(ac_ids))


def _http_execution_plan_issues(plan: dict) -> list[dict]:
    issues = []
    for index, item in enumerate(_http_verification_items(plan)):
        item_id = str(item.get("id") or f"verification[{index}]")
        ac_id = str(item.get("acId") or item.get("ac") or "").strip()
        if not ac_id:
            issues.append({"id": item_id, "field": "acId",
                           "message": "HTTP verification acId is required"})
        if item.get("stages") != ["local", "test-env"]:
            issues.append({"id": item_id, "field": "stages",
                           "message": "HTTP verification stages must be exactly [local, test-env]"})
        if item.get("internalMocksAllowed") is not False:
            issues.append({"id": item_id, "field": "internalMocksAllowed",
                           "message": "HTTP verification must set internalMocksAllowed=false"})
        if not str(item.get("command") or "").strip():
            issues.append({"id": item_id, "field": "command",
                           "message": "HTTP verification command is required"})
    return issues


def _safe_project_file(project_dir: Path, value: str) -> Optional[Path]:
    raw = str(value or "").strip()
    if not raw:
        return None
    root = project_dir.resolve()
    candidate = (root / raw).resolve()
    try:
        candidate.relative_to(root)
    except ValueError:
        return None
    return candidate


def check_g_http_1(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """Validate capability-derived HTTP scenarios without executing the runner."""
    gate_id = "G-HTTP-1"
    name = "HTTP 场景推导有效"
    plan = st.get("executionPlan") if isinstance(st.get("executionPlan"), dict) else {}
    policy_version = plan.get("scenarioPolicyVersion")
    if policy_version is None:
        return GateResult(gate_id, name, "blocker", True,
                          "legacy executionPlan 未启用场景推导策略（兼容读取）",
                          details={"skipped": True, "profile": "legacy"})
    if policy_version != 1:
        return GateResult(gate_id, name, "blocker", False,
                          f"不支持 scenarioPolicyVersion={policy_version}",
                          "使用当前支持的 scenarioPolicyVersion=1")
    http_items = _http_verification_items(plan)
    if not http_items:
        return GateResult(gate_id, name, "blocker", True,
                          "当前计划无 HTTP verification（not applicable）",
                          details={"skipped": True, "profile": "non-http"})

    from lib import http_scenario

    failures: list[dict] = []
    validated: list[dict] = []
    for index, item in enumerate(http_items):
        item_id = str(item.get("id") or f"verification[{index}]")
        manifest_value = str(item.get("scenarioManifest") or "").strip()
        manifest_path = _safe_project_file(project_dir, manifest_value)
        if manifest_path is None:
            failures.append({"id": item_id, "reason": "scenario-manifest-path",
                             "path": manifest_value})
            continue
        if not manifest_path.is_file():
            failures.append({"id": item_id, "reason": "scenario-manifest-absent",
                             "path": manifest_value})
            continue
        try:
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        except (OSError, ValueError) as exc:
            failures.append({"id": item_id, "reason": "scenario-manifest-invalid-json",
                             "path": manifest_value, "error": str(exc)})
            continue
        issues = http_scenario.validate_manifest(manifest)
        if issues:
            failures.append({"id": item_id, "reason": "scenario-contract-invalid",
                             "path": manifest_value, "issues": issues})
            continue
        required_acs = set(re.findall(r"AC[-_]?\d+", str(item.get("acId") or ""), re.I))
        scenario_acs = {
            str(ac_id) for scenario in manifest.get("scenarios") or []
            if isinstance(scenario, dict) for ac_id in scenario.get("acIds") or []
        }
        missing_acs = sorted(required_acs - scenario_acs)
        if missing_acs:
            failures.append({"id": item_id, "reason": "scenario-ac-coverage",
                             "path": manifest_value, "missingAcs": missing_acs})
            continue
        validated.append({"id": item_id, "path": manifest_value,
                          "scenarioCount": len(manifest.get("scenarios") or [])})
    if failures:
        return GateResult(gate_id, name, "blocker", False,
                          f"HTTP 场景推导合同无效（{len(failures)} 项）",
                          "补齐能力→状态→观察面→不变量→扰动→失败机制推导链及可重复命令",
                          details={"failures": failures})
    return GateResult(gate_id, name, "blocker", True,
                      f"HTTP 场景推导合同通过（{len(validated)} 项 verification）",
                      details={"validated": validated})


def _required_http_scenario_ids(project_dir: Path, plan: dict) -> list[str]:
    if plan.get("scenarioPolicyVersion") != 1:
        return []
    scenario_ids: set[str] = set()
    for item in _http_verification_items(plan):
        path = _safe_project_file(project_dir, str(item.get("scenarioManifest") or ""))
        if path is None or not path.is_file():
            continue
        try:
            manifest = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        scenario_ids.update(
            str(scenario.get("scenarioId") or "").strip()
            for scenario in manifest.get("scenarios") or [] if isinstance(scenario, dict)
        )
    return sorted(item for item in scenario_ids if item)


def check_g08(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-08 CodingPlan 14 门禁通过 — 解析 CodingPlan 文档 14 门禁表"""
    plan = st.get("executionPlan") if isinstance(st.get("executionPlan"), dict) else {}
    if plan.get("goal") or plan.get("changedPaths") or plan.get("verification"):
        missing = [name for name, value in (
            ("goal", plan.get("goal")),
            ("changedPaths", plan.get("changedPaths")),
            ("verification", plan.get("verification")),
        ) if not value]
        if missing:
            return GateResult(
                "G-08", "ExecutionPlan 门禁通过", "blocker", False,
                f"executionPlan 缺字段: {missing}",
                "补齐紧凑计划，不要创建 CodingPlan Markdown",
                details={"profile": "compact", "missing": missing},
            )
        http_issues = _http_execution_plan_issues(plan)
        if http_issues:
            return GateResult(
                "G-08", "ExecutionPlan HTTP 验证契约", "blocker", False,
                f"executionPlan HTTP verification 契约无效（{len(http_issues)} 项）",
                "接口 AC 必须声明 boundary=http、stages=[local,test-env]、internalMocksAllowed=false 和可执行 command",
                details={"profile": "compact", "reason": "http-verification-contract",
                         "issues": http_issues},
            )
        scenario_gate = check_g_http_1(project_dir, st, current_story)
        if not scenario_gate.pass_:
            return scenario_gate
        if not plan.get("approved"):
            return GateResult(
                "G-08", "ExecutionPlan 门禁通过", "blocker", False,
                "executionPlan 尚未获得用户确认",
                "在对话中展示紧凑计划并调用 execution.plan.approve",
                details={"profile": "compact", "approved": False},
            )
        return GateResult(
            "G-08", "ExecutionPlan 门禁通过", "blocker", True,
            "紧凑 executionPlan 完整且已确认",
            details={"profile": "compact", "approved": True,
                     "verificationCount": len(plan.get("verification") or []),
                     "riskCount": len(plan.get("risks") or [])},
        )
    cp, resolution = _resolve_codingplan_doc(project_dir, st, current_story)
    if resolution.get("errorCode"):
        return GateResult("G-08", "CodingPlan 门禁通过", "blocker", False,
                          "CodingPlan legacy Story 候选不唯一，拒绝猜测",
                          "显式传入当前 Work Item 或清理重复 CodingPlan",
                          details=resolution)
    if cp is None:
        return GateResult("G-08", "CodingPlan 门禁通过", "blocker", False,
                          "CodingPlan 文档不存在",
                          "先生成 Work Item CodingPlan（coding-process §A 调 coding-skill §5）",
                          details=resolution)

    content = cp.read_text(encoding="utf-8")

    if st.get("scale") == "微":
        heading_matches = list(
            re.finditer(r"^#{1,6}\s+(.+?)\s*$", content, re.MULTILINE)
        )
        sections = []
        for index, match in enumerate(heading_matches):
            body_end = (
                heading_matches[index + 1].start()
                if index + 1 < len(heading_matches)
                else len(content)
            )
            sections.append((
                match.group(1).strip().casefold(),
                content[match.end():body_end],
            ))
        missing_dimensions = [
            dimension
            for dimension, aliases in MICRO_CODINGPLAN_DIMENSIONS.items()
            if not any(
                alias.casefold() in heading and any(char.isalnum() for char in body)
                for alias in aliases
                for heading, body in sections
            )
        ]
        if missing_dimensions:
            return GateResult(
                "G-08", "CodingPlan 微任务门禁通过", "blocker", False,
                f"微任务 CodingPlan 缺轻量维度: {missing_dimensions}",
                "补充变更范围、实现顺序、风险/回滚与验证计划，不需要伪造大型 Plan 的 14 项",
                details={**resolution, "profile": "micro",
                         "missing_dimensions": missing_dimensions},
            )

        blocking_tokens = [
            token for token in ("❌", "TODO", "TBD", "待补充", "待确认")
            if token.casefold() in content.casefold()
        ]
        if blocking_tokens:
            return GateResult(
                "G-08", "CodingPlan 微任务门禁通过", "blocker", False,
                f"微任务 CodingPlan 仍含未闭环项: {blocking_tokens}",
                "闭环未完成项后重新确认 CodingPlan",
                details={**resolution, "profile": "micro",
                         "blocking_tokens": blocking_tokens},
            )
        return GateResult(
            "G-08", "CodingPlan 微任务门禁通过", "blocker", True,
            "微任务轻量门禁通过（范围/实现/风险回滚/验证齐全）",
            details={**resolution, "profile": "micro",
                     "dimensions": list(MICRO_CODINGPLAN_DIMENSIONS),
                     "file": str(cp)},
        )

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


def _resolve_g09_scope(project_dir: Path, st: dict, current_story: str):
    """Resolve a trustworthy work-item scope without consulting transient Git state."""
    from lib import verification_plan

    plan = st.get("verificationPlan")
    source = ""
    raw_paths = None
    since_fingerprint = ""
    expected_fingerprint = ""
    if isinstance(plan, dict):
        source = "verificationPlan.changedPaths"
        raw_paths = plan.get("changedPaths")
        since_fingerprint = str(plan.get("sinceFingerprint") or "")
        if str(plan.get("storyId") or "") != current_story:
            return None, source, "", "verification-plan storyId mismatch"
        if not isinstance(raw_paths, list) or not raw_paths:
            return None, source, "", "verification-plan changedPaths is empty or invalid"
        rebuilt = verification_plan.build_plan(
            project_dir, current_story, raw_paths, since_fingerprint,
            str(plan.get("workItem") or ""),
        )
        if plan.get("planFingerprint") != rebuilt["planFingerprint"]:
            return None, source, rebuilt["planFingerprint"], "verification-plan fingerprint mismatch"
        if plan.get("evidenceInputFingerprint") and plan.get("workItem"):
            expected_fingerprint = rebuilt["evidenceInputFingerprint"]
            if plan.get("evidenceInputFingerprint") != expected_fingerprint:
                return None, source, expected_fingerprint, "verification-plan evidence input fingerprint mismatch"
        else:
            expected_fingerprint = rebuilt["planFingerprint"]
    else:
        for field in ("changedPaths", "changedFiles"):
            value = st.get(field)
            if isinstance(value, list) and value:
                source = f"state.{field}"
                raw_paths = value
                break
        if raw_paths is None:
            return [], "full-repository", "", ""
        expected_fingerprint = verification_plan.build_plan(
            project_dir, current_story, raw_paths
        )["planFingerprint"]

    root = project_dir.resolve()
    normalized = []
    for raw in raw_paths:
        value = str(raw or "").strip().replace("\\", "/")
        relative = Path(value)
        if not value or relative.is_absolute() or ".." in relative.parts:
            return None, source, expected_fingerprint, f"unsafe scope path: {value or '<empty>'}"
        try:
            resolved = (root / relative).resolve(strict=True)
            normalized_path = resolved.relative_to(root)
        except (OSError, ValueError):
            return None, source, expected_fingerprint, f"scope path is missing or outside project: {value}"
        if not resolved.is_file():
            return None, source, expected_fingerprint, f"scope path is not a file: {value}"
        normalized.append(normalized_path.as_posix())
    normalized = sorted(set(normalized))
    if not normalized:
        return None, source, expected_fingerprint, "scope is empty after normalization"
    return normalized, source, expected_fingerprint, ""


def _finding_in_g09_scope(finding: dict, scope_paths: list[str]) -> bool:
    finding_path = str(finding.get("path") or "").replace("\\", "/").lstrip("./")
    return any(
        finding_path == scope or finding_path.startswith(scope.rstrip("/") + "/")
        for scope in scope_paths
    )


_GCODE1_EXTENSIONS = {
    ".java", ".kt", ".kts", ".xml", ".yaml", ".yml", ".properties",
    ".py", ".js", ".ts",
}
_GCODE1_EXCLUDED_DIRS = {
    ".git", ".idea", ".gradle", ".ae-sdd", ".auto-engineering", "ae-sdd-doc",
    "node_modules", "target", "build", "dist", "__pycache__",
    ".venv", "venv", ".tox", "site-packages", "__tests__",
}


def _is_gcode1_test_path(value: str) -> bool:
    normalized = "/" + value.replace("\\", "/").lower().lstrip("/")
    if any(marker in normalized for marker in (
        "/src/test/", "/src/integrationtest/", "/src/testfixtures/", "/test/", "/tests/",
        "/__tests__/",
    )):
        return True
    path = Path(value)
    stem = path.stem
    stem_lower = stem.lower()
    suffix = path.suffix.lower()
    if suffix == ".py":
        return stem_lower == "test" or stem_lower.startswith("test_") or stem_lower.endswith("_test")
    if suffix in {".js", ".ts"}:
        return stem_lower.endswith((".test", ".spec"))
    if suffix in {".java", ".kt", ".kts"}:
        return stem_lower.endswith(("test", "tests", "spec")) or stem.endswith("IT")
    return False


def _gcode1_production_scope(scope_paths: list[str]) -> list[str]:
    """Keep only production inputs understood by coding_authenticity_scan.py."""
    production = []
    for value in scope_paths:
        path = Path(value)
        if path.suffix.lower() not in _GCODE1_EXTENSIONS:
            continue
        if any(part.lower() in _GCODE1_EXCLUDED_DIRS for part in path.parts):
            continue
        if _is_gcode1_test_path(value):
            continue
        production.append(value)
    return production


def _validate_scanner_findings(project_dir: Path, findings) -> tuple[bool, list[dict], str]:
    """Reject malformed/unrooted finding paths before any scope filtering."""
    if not isinstance(findings, list):
        return False, [], "findings is not a list"
    root = project_dir.resolve()
    validated = []
    for finding in findings:
        if not isinstance(finding, dict):
            return False, [], "finding is not an object"
        severity = finding.get("severity")
        if severity not in {"BLOCKER", "WARN"}:
            return False, [], f"unsupported finding severity: {severity!r}"
        raw = str(finding.get("path") or "").strip().replace("\\", "/")
        relative = Path(raw)
        if not raw or relative.is_absolute() or ".." in relative.parts:
            return False, [], f"unsafe finding path: {raw or '<empty>'}"
        try:
            resolved = (root / relative).resolve(strict=True)
            normalized = resolved.relative_to(root).as_posix()
        except (OSError, ValueError):
            return False, [], f"finding path is missing or outside project: {raw}"
        if not resolved.is_file():
            return False, [], f"finding path is not a regular file: {raw}"
        item = dict(finding)
        item["path"] = normalized
        validated.append(item)
    return True, validated, ""


def _validate_scanned_paths(project_dir: Path, values) -> tuple[bool, list[str], str]:
    """Validate scanner coverage attestation as unique project-relative files."""
    if not isinstance(values, list):
        return False, [], "scannedPaths is not a list"
    root = project_dir.resolve()
    normalized_paths = []
    seen = set()
    for value in values:
        if not isinstance(value, str):
            return False, [], "scannedPaths entry is not a string"
        raw = value.strip().replace("\\", "/")
        relative = Path(raw)
        if not raw or relative.is_absolute() or ".." in relative.parts:
            return False, [], f"unsafe scanned path: {raw or '<empty>'}"
        try:
            resolved = (root / relative).resolve(strict=True)
            normalized = resolved.relative_to(root).as_posix()
        except (OSError, ValueError):
            return False, [], f"scanned path is missing or outside project: {raw}"
        if not resolved.is_file():
            return False, [], f"scanned path is not a regular file: {raw}"
        if _gcode1_production_scope([normalized]) != [normalized]:
            return False, [], f"scanned path is not eligible production code: {raw}"
        identity = normalized.casefold() if sys.platform == "win32" else normalized
        if identity in seen:
            return False, [], f"duplicate scanned path: {raw}"
        seen.add(identity)
        normalized_paths.append(normalized)
    return True, sorted(normalized_paths), ""


def _validate_scanner_report_schema(project_dir: Path, report: dict,
                                    scanned_paths: list[str],
                                    findings: list[dict]) -> tuple[bool, str]:
    root_value = report.get("root")
    if not isinstance(root_value, str) or not root_value.strip():
        return False, "root is missing or not a string"
    try:
        if Path(root_value).resolve() != project_dir.resolve():
            return False, "root does not match project root"
    except OSError:
        return False, "root cannot be resolved"

    code_files = report.get("codeFiles")
    coding_reports = report.get("codingReports")
    if type(code_files) is not int or code_files < 0:
        return False, "codeFiles must be a non-negative integer"
    if code_files != len(scanned_paths):
        return False, "codeFiles does not match scannedPaths"
    if type(coding_reports) is not int or coding_reports < 0:
        return False, "codingReports must be a non-negative integer"

    stats = report.get("reportStats")
    if not isinstance(stats, dict):
        return False, "reportStats is missing or not an object"
    expected = {
        "codeFiles": code_files,
        "codingReports": coding_reports,
        "blockerFindings": sum(
            1 for finding in findings if finding.get("severity") == "BLOCKER"
        ),
        "warnFindings": sum(
            1 for finding in findings if finding.get("severity") == "WARN"
        ),
    }
    for field, value in expected.items():
        actual = stats.get(field)
        if type(actual) is not int or actual < 0:
            return False, f"reportStats.{field} must be a non-negative integer"
        if actual != value:
            return False, f"reportStats.{field} is inconsistent"
    return True, ""


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

    scope_paths, scope_source, scope_fingerprint, scope_error = _resolve_g09_scope(
        project_dir, st, current_story
    )
    if scope_error:
        return GateResult(
            "G-09", "测试真实性扫描通过", "blocker", False,
            f"work-item scope 无效: {scope_error}",
            "重新生成 VerificationPlan 并核对 changedPaths",
            details={"scanned": False, "scopeMode": "work-item",
                     "scopeSource": scope_source, "scopeStatus": "BLOCK_SCOPE_INVALID"},
        )
    scope_mode = "work-item" if scope_paths else "full-repository"
    if scope_paths and isinstance(st.get("verificationPlan"), dict):
        from lib import evidence
        evidence_ok, evidence_reason = evidence.validate_g09_manifest(
            project_dir, current_story, scope_fingerprint, scope_paths
        )
        if not evidence_ok:
            return GateResult(
                "G-09", "测试真实性扫描通过", "blocker", False,
                f"G-09 evidence 完整性校验失败: {evidence_reason}",
                "重跑当前 work-item 测试并重新记录 evidence",
                details={"scanned": False, "scopeMode": scope_mode,
                         "scopeSource": scope_source, "scopePaths": scope_paths,
                         "scopeStatus": "BLOCK_EVIDENCE_INVALID",
                         "evidenceReason": evidence_reason},
            )

    http_ac_ids = _http_verification_ac_ids(
        st.get("executionPlan") if isinstance(st.get("executionPlan"), dict) else {}
    )
    http_evidence_details = None
    if http_ac_ids:
        from lib import evidence
        execution_plan = st.get("executionPlan") if isinstance(st.get("executionPlan"), dict) else {}
        http_ok, http_reason, http_evidence_details = evidence.validate_http_acceptance_manifest(
            project_dir, current_story, http_ac_ids, scope_fingerprint,
            _required_http_scenario_ids(project_dir, execution_plan),
        )
        if not http_ok:
            return GateResult(
                "G-09", "HTTP 双阶段验收证据", "blocker", False,
                f"HTTP acceptance evidence 校验失败: {http_reason}",
                "先完成本地真实 HTTP，再以同一 buildId 完成测试环境 HTTP，并重新记录 evidence",
                details={"scanned": False, "scopeMode": scope_mode,
                         "scopeSource": scope_source, "scopePaths": scope_paths,
                         "scopeStatus": "BLOCK_HTTP_EVIDENCE_INVALID",
                         "evidenceReason": http_reason,
                         "httpAcs": http_ac_ids,
                         "httpEvidence": http_evidence_details},
            )

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
            capture_output=True, text=True, timeout=_GATE_SUBPROCESS_LIMIT_SECONDS,
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

    findings = report.get("findings", [])
    if scope_paths:
        findings = [finding for finding in findings
                    if _finding_in_g09_scope(finding, scope_paths)]
        blockers = sum(1 for finding in findings if finding.get("severity") == "BLOCKER")
        status = "PASS" if blockers == 0 else "FAIL"
        java_test_files = sum(
            1 for value in scope_paths
            if value.lower().endswith(".java") and "/test/" in f"/{value.lower()}"
        )
    else:
        status = report.get("status", "UNKNOWN")
        java_test_files = report.get("javaTestFiles", 0)
        blockers = sum(1 for finding in findings if finding.get("severity") == "BLOCKER")
    n_total = len(findings)
    scope_details = {"scopeMode": scope_mode, "scopeSource": scope_source,
                     "scopePaths": scope_paths, "scopeStatus": "VERIFIED"}
    if http_evidence_details is not None:
        scope_details["httpEvidence"] = http_evidence_details

    # 有 findings / BLOCKER → 直接 fail（无论 phase）
    if status != "PASS" or blockers > 0:
        return GateResult("G-09", "测试真实性扫描通过", "blocker", False,
                          f"扫描失败：{n_total} findings / {blockers} BLOCKER",
                          f"修复测试代码中的 8 类禁止（{scanner.name}）",
                          details={"scanned": True, "n_findings": n_total, "n_blockers": blockers,
                                   "status": status, "n_test_files": java_test_files,
                                   **scope_details})

    # 0 测试文件：
    # - pre-coding → stub（还没到写测试的阶段，扫描无对象不算 pass）
    # - ≥ coding → warn（应该写测试但没写）
    if java_test_files == 0:
        if phase in PRE_CODING_PHASES:
            return GateResult("G-09", "测试真实性扫描通过", "blocker", True,
                              f"phase = {phase}（pre-coding，扫描无对象，按 stub 算）",
                              action="进入 coding 阶段后此门禁生效",
                              details={"scanned": True, "skipped": True, "stub": True,
                                       "current_phase": phase, "n_test_files": 0,
                                       **scope_details})
        else:
            return GateResult("G-09", "测试真实性扫描通过", "warn", True,
                              f"phase = {phase} 但 0 测试文件（应编写测试）",
                              action="确认是否漏写测试代码",
                              details={"scanned": True, "n_findings": 0, "n_test_files": 0,
                                       "current_phase": phase, "stub": False,
                                       **scope_details})

    # 有测试文件 + 0 BLOCKER → 真 pass
    # 🆕 v3.4.0 test-verifier 独立性校验（建议书3 B2-7）：测试真实性报告应带独立 session_id
    verifier_warning = _check_test_verifier_independence(project_dir, current_story)

    if verifier_warning:
        return GateResult("G-09", "测试真实性扫描通过", "warn", True,
                          f"扫描通过：{n_total} findings / 0 BLOCKER（{java_test_files} 测试文件）。⚠️ {verifier_warning}",
                          action="test-verifier sub-agent 报告须带独立 session_id（≠ 主 agent）",
                          details={"scanned": True, "n_findings": n_total, "n_blockers": 0,
                                   "n_test_files": java_test_files,
                                   "verifier_warning": verifier_warning,
                                   **scope_details})

    return GateResult("G-09", "测试真实性扫描通过", "blocker", True,
                      f"扫描通过：{n_total} findings / 0 BLOCKER（{java_test_files} 测试文件）",
                      details={"scanned": True, "n_findings": n_total, "n_blockers": 0,
                               "n_test_files": java_test_files, **scope_details})


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

    # Scoped Coding authenticity reuses G-09's persisted plan and evidence
    # chain. Missing or empty scope retains the legacy full-repository scan.
    scope_paths: list[str] = []
    scope_source = "full-repository"
    scope_fingerprint = ""
    plan = st.get("verificationPlan")
    if isinstance(plan, dict) and plan.get("changedPaths"):
        scope_paths, scope_source, scope_fingerprint, scope_error = _resolve_g09_scope(
            project_dir, st, current_story
        )
        if scope_error:
            return GateResult(
                "G-CODE-1", "Coding 真实性扫描通过", "blocker", False,
                f"work-item scope 无效: {scope_error}",
                "重新生成 VerificationPlan 并核对 changedPaths",
                details={"scanned": False, "scopeMode": "work-item",
                         "scopeSource": scope_source, "scopeStatus": "BLOCK_SCOPE_INVALID"},
            )
        from lib import evidence
        evidence_ok, evidence_reason = evidence.validate_g09_manifest(
            project_dir, current_story, scope_fingerprint, scope_paths
        )
        if not evidence_ok:
            return GateResult(
                "G-CODE-1", "Coding 真实性扫描通过", "blocker", False,
                f"G-CODE-1 evidence 完整性校验失败: {evidence_reason}",
                "重跑当前 work-item 测试并重新记录 evidence",
                details={"scanned": False, "scopeMode": "work-item",
                         "scopeSource": scope_source, "scopePaths": scope_paths,
                         "scopeStatus": "BLOCK_EVIDENCE_INVALID",
                         "evidenceReason": evidence_reason},
            )
        production_scope = _gcode1_production_scope(scope_paths)
        if not production_scope:
            return GateResult(
                "G-CODE-1", "Coding 真实性扫描通过", "blocker", False,
                "work-item scope 不含可扫描的生产代码",
                "核对 VerificationPlan changedPaths 或使用全仓扫描",
                details={"scanned": False, "scopeMode": "work-item",
                         "scopeSource": scope_source, "scopePaths": scope_paths,
                         "scopeStatus": "BLOCK_NO_PRODUCTION_SCOPE"},
            )
        scope_paths = production_scope
    scope_mode = "work-item" if scope_paths else "full-repository"
    scope_details = {"scopeMode": scope_mode, "scopeSource": scope_source,
                     "scopePaths": scope_paths,
                     "scopeStatus": "VERIFIED" if scope_paths else "FULL_REPOSITORY"}

    scanner = _locate_coding_scanner(master_source)
    if scanner is None:
        return GateResult(
            "G-CODE-1", "Coding authenticity scan", "blocker", False,
            "coding_authenticity_scan.py is unavailable",
            "Restore the distributed scanner and rerun G-CODE-1",
            details={"scanned": False, **scope_details,
                     "scopeStatus": "BLOCK_SCAN_INVALID"},
        )

    try:
        result = runtime_exec.run_command(
            [sys.executable, str(scanner), "--root", str(project_dir), "--format", "json"],
            capture_output=True, text=True, timeout=_GATE_SUBPROCESS_LIMIT_SECONDS,
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

    report_status = report.get("status") if isinstance(report, dict) else None
    valid_status_exit = (
        (report_status == "PASS" and result.returncode == 0)
        or (report_status == "FAIL" and result.returncode == 1)
    )
    if not valid_status_exit:
        return GateResult(
            "G-CODE-1", "Coding authenticity scan", "blocker", False,
            f"scanner result invalid: exit={result.returncode}, status={report_status}",
            "Check coding_authenticity_scan.py exit code and JSON status",
            details={"scanned": False, **scope_details,
                     "scopeStatus": "BLOCK_SCAN_INVALID"},
        )
    findings_ok, findings, findings_error = _validate_scanner_findings(
        project_dir, report.get("findings")
    )
    if not findings_ok:
        return GateResult(
            "G-CODE-1", "Coding authenticity scan", "blocker", False,
            f"scanner findings invalid: {findings_error}",
            "Check coding_authenticity_scan.py finding paths",
            details={"scanned": False, **scope_details,
                     "scopeStatus": "BLOCK_SCAN_INVALID"},
        )
    report_blockers = sum(
        1 for finding in findings if finding.get("severity") == "BLOCKER"
    )
    if (report_status == "PASS" and report_blockers) or (
        report_status == "FAIL" and not report_blockers
    ):
        return GateResult(
            "G-CODE-1", "Coding authenticity scan", "blocker", False,
            f"scanner status/findings mismatch: status={report_status}, blockers={report_blockers}",
            "Check coding_authenticity_scan.py report consistency",
            details={"scanned": False, **scope_details,
                     "scopeStatus": "BLOCK_SCAN_INVALID"},
        )
    scanned_ok, scanned_paths, scanned_error = _validate_scanned_paths(
        project_dir, report.get("scannedPaths")
    )
    if not scanned_ok:
        return GateResult(
            "G-CODE-1", "Coding authenticity scan", "blocker", False,
            f"scanner coverage invalid: {scanned_error}",
            "Check coding_authenticity_scan.py scannedPaths attestation",
            details={"scanned": False, **scope_details,
                     "scopeStatus": "BLOCK_SCAN_INVALID"},
        )
    schema_ok, schema_error = _validate_scanner_report_schema(
        project_dir, report, scanned_paths, findings
    )
    if not schema_ok:
        return GateResult(
            "G-CODE-1", "Coding authenticity scan", "blocker", False,
            f"scanner report schema invalid: {schema_error}",
            "Check coding_authenticity_scan.py report attestation",
            details={"scanned": False, **scope_details,
                     "scopeStatus": "BLOCK_SCAN_INVALID"},
        )
    if scope_paths:
        missing_scope = sorted(set(scope_paths) - set(scanned_paths))
        if missing_scope:
            return GateResult(
                "G-CODE-1", "Coding authenticity scan", "blocker", False,
                f"scanner coverage incomplete: {missing_scope}",
                "Rerun coding_authenticity_scan.py for the complete production scope",
                details={"scanned": False, **scope_details,
                         "scannedPaths": scanned_paths,
                         "scopeStatus": "BLOCK_SCAN_INVALID"},
            )
    scope_details["scannedPaths"] = (
        sorted(set(scope_paths) & set(scanned_paths)) if scope_paths else scanned_paths
    )
    if scope_paths:
        findings = [finding for finding in findings
                    if _finding_in_g09_scope(finding, scope_paths)]
        status = "PASS" if not any(
            finding.get("severity") == "BLOCKER" for finding in findings
        ) else "FAIL"
        code_files = len(set(scope_paths) & set(scanned_paths))
        coding_reports = 0
    else:
        status = report_status
        code_files = len(scanned_paths)
        coding_reports = report.get("codingReports", 0)

    baseline_payload = None
    baseline_error = None
    baseline_mod = None
    if not scope_paths:
        try:
            from lib import baseline as baseline_mod
            baseline_payload, baseline_error = baseline_mod.load(project_dir, "G-CODE-1")
        except Exception as exc:
            baseline_error = f"baseline-error: {exc}"
    if baseline_error == "tampered":
        return GateResult(
            "G-CODE-1", "Coding 真实性扫描通过", "blocker", False,
            "G-CODE-1 baseline 完整性校验失败（文件可能被篡改）",
            "恢复 baseline 或显式重新创建并记录用户批准",
            details={"scanned": True, "baselineStatus": "BLOCK_BASELINE_INVALID"},
        )

    blockers = sum(1 for f in findings if f.get("severity") == "BLOCKER")
    n_total = len(findings)

    if status != "PASS" or blockers > 0:
        # P1: an explicit, integrity-checked baseline can separate repository
        # debt from Story delta. Missing/tampered baselines retain legacy full
        # blocking behavior; baseline creation is never automatic here.
        try:
            if baseline_mod is not None and baseline_payload is not None and baseline_error is None:
                touched = st.get("changedPaths") or st.get("changedFiles") or []
                delta = baseline_mod.compare(
                    baseline_payload,
                    findings,
                    ruleset_fingerprint=baseline_payload.get("rulesetFingerprint", ""),
                    touched_paths=touched,
                )
                if delta["status"] == "PASS_WITH_BASELINE_DEBT" and not delta["touchedDebt"]:
                    return GateResult(
                        "G-CODE-1", "Coding 真实性扫描通过", "warn", True,
                        f"增量扫描通过，保留 {delta['baseline']} 项 baseline debt（新增 blocker=0）",
                        "继续治理 baseline debt；本 Story 不因历史债务阻断",
                        details={"scanned": True, "baselineStatus": delta["status"],
                                 "baselineFindings": delta["baseline"], "currentFindings": delta["current"],
                                 "newFindings": len(delta["new"]), "touchedDebt": 0},
                    )
                if delta["status"] == "BLOCK_TOUCHED_DEBT":
                    return GateResult(
                        "G-CODE-1", "Coding 真实性扫描通过", "blocker", False,
                        f"Story 修改触及 {len(delta['touchedDebt'])} 项 baseline debt",
                        "修复被触及的历史债务，或提交有证据的用户授权",
                        details={"scanned": True, "baselineStatus": delta["status"],
                                 "baselineFindings": delta["baseline"], "currentFindings": delta["current"],
                                 "newFindings": len(delta["new"]), "touchedDebt": len(delta["touchedDebt"])},
                    )
        except Exception:
            # Baseline support is additive; a malformed optional baseline must
            # fall back to the legacy full-scan blocker path.
            pass
        blocker_rules = sorted({f.get("rule") for f in findings
                                if f.get("severity") == "BLOCKER"})
        return GateResult("G-CODE-1", "Coding 真实性扫描通过", "blocker", False,
                          f"Coding 真实性扫描发现 {blockers} 个 BLOCKER（共 {n_total} 项）：{blocker_rules}",
                          "修复 Coding 反模式命中项，或在 CodeReview 中显式评审通过",
                          details={"scanned": True, "n_findings": n_total,
                                   "n_blockers": blockers, "status": status,
                                   "n_code_files": code_files,
                                   "n_coding_reports": coding_reports,
                                   "blocker_rules": blocker_rules, **scope_details})

    if code_files == 0:
        if phase in PRE_CODING_PHASES:
            return GateResult("G-CODE-1", "Coding 真实性扫描通过", "blocker", True,
                              f"phase = {phase}（pre-coding，扫描无对象，按 stub 算）",
                              action="进入 coding 阶段后此门禁生效",
                              details={"scanned": True, "skipped": True, "stub": True,
                                       "current_phase": phase, "n_code_files": 0,
                                       "n_coding_reports": coding_reports, **scope_details})
        return GateResult("G-CODE-1", "Coding 真实性扫描通过", "warn", True,
                          f"phase = {phase} 但 0 个生产代码文件（请确认是否漏扫项目根）",
                          action="确认 --project / cwd 是否指向服务根或仓库根",
                          details={"scanned": True, "n_findings": n_total,
                                   "n_code_files": 0, "n_coding_reports": coding_reports,
                                   "current_phase": phase, "stub": False, **scope_details})

    return GateResult("G-CODE-1", "Coding 真实性扫描通过", "blocker", True,
                      f"Coding 真实性扫描通过（{code_files} 个代码文件，{coding_reports} 份 Coding 报告，0 BLOCKER，{n_total} WARN）",
                      details={"scanned": True, "n_findings": n_total,
                               "n_blockers": 0, "n_code_files": code_files,
                               "n_coding_reports": coding_reports, **scope_details})


# ─── G-13：RA ↔ DR ↔ Story ↔ Task ↔ Coding 六层引用追溯 ──────────────────────
def _g13_trace_mode(st: dict, current_story: str) -> tuple[str, str]:
    entry_node = str(st.get("entryNode") or "").upper()
    work_item = _work_item_from_state(st)
    is_bug = entry_node == "BUG" or work_item.upper().startswith("BUG") \
        or work_item.upper().startswith("BUG-")
    if is_bug and st.get("scale") == "微":
        explicit_parent = str(st.get("parentStory") or "")
        parent_story = explicit_parent or (
            current_story if current_story.upper().startswith("STORY-") else ""
        )
        if parent_story:
            return "inherited-parent-story", parent_story
        return "minimal-bug-trace", ""
    return "strict-dr", ""


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
    trace_mode, parent_story = _g13_trace_mode(st, current_story)
    if trace_mode == "inherited-parent-story":
        return GateResult(
            "G-13", "全链路对称性核查通过", "blocker", True,
            f"BUG follow-up 继承父 Story 追溯：{parent_story}",
            details={"traceMode": trace_mode,
                     "parentStory": parent_story,
                     "workItem": _work_item_from_state(st)},
        )
    if trace_mode == "minimal-bug-trace":
        return GateResult(
            "G-13", "全链路对称性核查通过", "blocker", True,
            "standalone micro BUG 使用最小追溯，不要求虚构 DR/Story",
            details={"traceMode": trace_mode,
                     "workItem": _work_item_from_state(st)},
        )
    if not current_story:
        return GateResult("G-13", "全链路对称性核查通过", "blocker", False,
                          "state.currentStory 为空",
                          details={"traceMode": trace_mode})

    design = _g13_design_root(project_dir)
    if not design.is_dir():
        return GateResult("G-13", "全链路对称性核查通过", "blocker", False,
                          f"design/ 与 ae-sdd-doc/ 均不存在",
                          f"先生成设计文档",
                          details={"traceMode": trace_mode})

    issues: list[str] = []
    ra_layer_detail: dict = {"present": False, "files": 0}
    story_entry_dr_exempt = (
        str(st.get("entryNode") or "").upper() == "STORY"
        and st.get("scale") == "中"
    )
    dr_layer_detail = {
        "status": "EXEMPT_STORY_ENTRY" if story_entry_dr_exempt else "REQUIRED",
        "exempt": story_entry_dr_exempt,
    }

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

        drs = _iter_dr_docs(design)
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
        # 找 design/ 下的所有 DR（rglob 递归子目录，与 G-01 一致）
        drs = _iter_dr_docs(design)
        if not drs and not story_entry_dr_exempt:
            issues.append("无 DR 文档可追溯（design/ 目录无 *DR*.md）")
        elif drs:
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

    compact_process = isinstance(st.get("executionPlan"), dict) and bool(
        st.get("executionPlan", {}).get("goal")
    )

    # 2. Task → Story 引用追溯（新流程默认不生成 Task Markdown）
    tasks = _find_task_docs(project_dir, current_story)
    phase_requires_completed_chain = (
        st.get("phase") in {"code-reviewed", "completed"} and not compact_process
    )
    if phase_requires_completed_chain and not tasks:
        issues.append(f"code-reviewed 链路缺少 Task 文档：{current_story}.md 或 {current_story}-task-*.md")
    for t in tasks:
        task_content = t.read_text(encoding="utf-8")
        if current_story not in task_content:
            issues.append(f"Task 文档未引用 Story ID {current_story}：{t.name}")

    # 3. Coding Report → Task 引用追溯（仅 legacy；新流程使用 executionPlan/evidence）
    work_item = _work_item_from_state(st) or current_story
    coding_report = _find_report_doc(
        project_dir, current_story,
        category="Coding",
        patterns=[f"{current_story}-CodingReport.md",  # 🆕 v3.10.1 原地更新（主）
                  f"{current_story}-CodingReport-v*-r*.md",  # 兼容旧版本化
                  f"{current_story}-Coding-Report-v*-r*.md"],
        legacy_suffixes=["-CodingReport.md", "-Coding-Report.md"],  # design/ 兼容
        work_item=work_item,
    )
    if coding_report is not None:
        cr_content = coding_report.read_text(encoding="utf-8")
        for t in tasks:
            # 🆕 v3.10.5 BUG5：v3.10 砍 Task 后主产物 {story}.md，其 stem==current_story，
            # 原 t.stem in cr_content 校验永真（cr 必然含 story id）。改为：主布局（stem==story）
            # 时校验 cr 是否含 task 文件名 t.name 或显式 task 段落标记；旧布局（-task-*，stem≠story）
            # 保持 t.stem in cr_content 逻辑。
            if t.stem == current_story:
                # v3.10 主布局：校验文件名或 task 段落标记（避免 story id 永真）
                linked = (t.name in cr_content
                          or f"Task：{t.stem}" in cr_content
                          or f"Task:{t.stem}" in cr_content)
            else:
                # 旧布局 -task-*.md：stem 形如 STORY-001-task-001
                linked = t.stem in cr_content
            if not linked:
                issues.append(f"Coding Report 未引用 Task：{t.name}")
    elif phase_requires_completed_chain:
        issues.append(f"code-reviewed 链路缺少 Coding Report：{current_story}")

    # 4. CodeReview → Story 引用追溯（仅 legacy；新流程使用 review findings/status）
    code_review = _find_report_doc(
        project_dir, current_story,
        category="CR",
        patterns=[f"{current_story}-CodeReview.md",  # 🆕 v3.10.1 原地更新（主），与 G-12 对齐
                  f"{current_story}-CodeReview-v*-r*.md"],  # 兼容旧版本化
        legacy_suffixes=["-CodeReview.md"],
        work_item=work_item,
    )
    if code_review is not None:
        cv_content = code_review.read_text(encoding="utf-8")
        if current_story not in cv_content:
            issues.append(f"CodeReview 报告未引用 Story ID {current_story}")
    elif phase_requires_completed_chain:
        issues.append(f"code-reviewed 链路缺少 CodeReview：{current_story}")

    if compact_process and st.get("phase") in {"code-reviewed", "completed"}:
        plan = st.get("executionPlan") or {}
        review = st.get("review") or {}
        if not plan.get("approved"):
            issues.append("executionPlan 尚未确认")
        if review.get("status") != "passed":
            issues.append("review.status 尚未通过")

    if issues:
        return GateResult("G-13", "全链路对称性核查通过", "blocker", False,
                          f"链路追溯发现 {len(issues)} 个问题：{issues[0]}" + ("..." if len(issues) > 1 else ""),
                          "修复文档间的引用关系",
                          details={"issues": issues, "n_issues": len(issues),
                                   "ra_layer": ra_layer_detail,
                                   "entryNode": st.get("entryNode"),
                                   "dr_layer": dr_layer_detail,
                                   "traceMode": trace_mode})

    n_drs = len(_iter_dr_docs(design))
    if compact_process:
        layer_note = "核心文档与机器证据追溯完整（RA/DR/Story + executionPlan/evidence/review）"
    else:
        layer_note = "六层追溯完整（RA ↔ DR ↔ Story ↔ Task ↔ Coding Report ↔ CodeReview）" \
            if ra_layer_detail["present"] else \
            "五层追溯完整（DR ↔ Story ↔ Task ↔ Coding Report ↔ CodeReview，RA 层未生成/豁免）"
    return GateResult("G-13", "全链路对称性核查通过", "blocker", True,
                      layer_note,
                      details={"current_story": current_story,
                               "n_tasks": len(tasks),
                               "n_drs": n_drs,
                               "ra_layer": ra_layer_detail,
                               "entryNode": st.get("entryNode"),
                               "dr_layer": dr_layer_detail,
                               "traceMode": trace_mode})


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
def _iter_ra_files(project_dir: Path) -> list[Path]:
    """枚举项目内 RA 文档（兼容新路径 ae-sdd-doc/ 与旧路径 design/）。"""
    if not project_dir or not Path(project_dir).is_dir():
        return []
    try:
        return list(resolve_ra_scan_scope(Path(project_dir)).files)
    except (OSError, ValueError):
        return []


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


def _relative_ra_path(path: Path, project_dir: Path) -> str:
    try:
        return path.resolve().relative_to(project_dir.resolve()).as_posix()
    except ValueError:
        return path.resolve().as_posix()


def _ra_binding_candidates(st: dict, current_story: str) -> list[tuple[str, str]]:
    candidates: list[tuple[str, str]] = []

    story_id = current_story or st.get("activeStory") or st.get("currentStory") or ""
    story_states = st.get("storyStates")
    if story_id and isinstance(story_states, dict):
        story_state = story_states.get(story_id)
        if isinstance(story_state, dict):
            story_path = story_state.get("raDocPath")
            if isinstance(story_path, str) and story_path.strip():
                candidates.append((f"storyStates.{story_id}.raDocPath", story_path.strip()))

    dr_states = st.get("drStates")
    if story_id and isinstance(dr_states, dict):
        for dr_id, dr_state in dr_states.items():
            if not isinstance(dr_state, dict):
                continue
            nested_story_states = dr_state.get("storyStates")
            if not isinstance(nested_story_states, dict):
                continue
            story_state = nested_story_states.get(story_id)
            if not isinstance(story_state, dict):
                continue
            story_path = story_state.get("raDocPath")
            if isinstance(story_path, str) and story_path.strip():
                candidates.append((
                    f"drStates.{dr_id}.storyStates.{story_id}.raDocPath",
                    story_path.strip(),
                ))

    top_level = st.get("raDocPath")
    if isinstance(top_level, str) and top_level.strip():
        candidates.append(("state.raDocPath", top_level.strip()))

    return candidates


def _resolve_selected_ra(
    project_dir: Path,
    st: dict,
    current_story: str,
) -> tuple[Optional[Path], dict]:
    """Resolve the single authoritative RA shared by all G-RA gates."""
    project_root = project_dir.resolve()
    discovered = _iter_ra_files(project_root)
    invalid_bindings: list[dict[str, str]] = []

    for source, raw_path in _ra_binding_candidates(st, current_story):
        candidate = Path(raw_path)
        if not candidate.is_absolute():
            candidate = project_root / candidate
        candidate = candidate.resolve()
        try:
            candidate.relative_to(project_root)
        except ValueError:
            invalid_bindings.append({"source": source, "path": raw_path, "reason": "outside-project"})
            continue
        if not candidate.is_file():
            invalid_bindings.append({"source": source, "path": raw_path, "reason": "missing-file"})
            continue
        if candidate.suffix.casefold() != ".md":
            invalid_bindings.append({"source": source, "path": raw_path, "reason": "not-markdown"})
            continue
        is_formal, reason = classify_formal_ra(candidate, project_root)
        if not is_formal:
            invalid_bindings.append({"source": source, "path": raw_path, "reason": reason})
            continue
        return candidate, {
            "selected_file": _relative_ra_path(candidate, project_root),
            "selection_source": source,
            "scope_mode": "file",
            "ra_files": 1,
            "candidate_count": len(discovered),
            "invalid_bindings": invalid_bindings,
        }

    if discovered:
        selected = _select_latest_ra(discovered)
        return selected, {
            "selected_file": _relative_ra_path(selected, project_root),
            "selection_source": "latest-formal-ra",
            "scope_mode": "file",
            "ra_files": 1,
            "candidate_count": len(discovered),
            "invalid_bindings": invalid_bindings,
        }

    return None, {
        "selected_file": None,
        "selection_source": "none",
        "scope_mode": "file",
        "ra_files": 0,
        "candidate_count": 0,
        "invalid_bindings": invalid_bindings,
    }


def check_ra_required(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-RA-1 RA 文档存在（SKILL.md G-RA 规则 1/5/6/7）。

    规则 1：进 dr/story/task-generate 前必须存在 RA 文档。
    规则 5：RA 距今 ≤ 30 天（超期 → warn，不阻断）。
    规则 6/7：微任务/BUG 豁免（phase 标识或无下游需求时不阻断）。
    """
    name = "RA 文档存在"
    phase = st.get("phase", "initialized")

    selected_ra, ra_resolution = _resolve_selected_ra(project_dir, st, current_story)

    # 规则 1：RA 文档存在
    if selected_ra is None:
        # pre-RA 阶段（还没开始需求分析）→ stub，不阻断
        pre_ra_phases = {"initialized", "ra-generated"}
        if phase in pre_ra_phases:
            return GateResult("G-RA-1", name, "blocker", True,
                              "pre-RA 阶段，RA 文档尚未生成（stub 通过）",
                              details={**ra_resolution, "stub": True, "phase": phase, "ra_files": 0})
        return GateResult("G-RA-1", name, "blocker", False,
                          f"未找到 RA 文档（phase={phase}，已进入下游节点）",
                          "运行 `ae-sdd gate ra-required --fix` 或触发 requirement-analysis-skill 生成 RA",
                          details={**ra_resolution, "phase": phase, "ra_files": 0})

    # 规则 5：RA 距今 ≤ 30 天（取最新一份的修改时间）
    latest = selected_ra
    mtime = datetime.fromtimestamp(latest.stat().st_mtime, tz=timezone.utc)
    now = datetime.now(tz=timezone.utc)
    age_days = (now - mtime).days

    if age_days > 30:
        return GateResult("G-RA-1", name, "warn", True,
                          f"RA 文档距今 {age_days} 天（超 30 天），建议重审：{latest.name}",
                          "重审 RA 是否仍反映当前需求",
                          details={**ra_resolution, "ra_files": 1, "latest": latest.name,
                                   "age_days": age_days, "warn_only": True})

    return GateResult("G-RA-1", name, "blocker", True,
                      f"RA 文档存在（1 份，选中 {latest.name}，{age_days} 天前）",
                      details={**ra_resolution, "ra_files": 1, "latest": latest.name,
                               "age_days": age_days})


def check_ra_dimensions(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-RA-2 RA 8 维度完整 + RAModel 12 维（SKILL.md G-RA 规则 2）。

    规则 2：RA 文档必须含 8 个核心维度。同时检查 RAModel 12 维决策记录（RA-G02）。
    """
    name = "RA 8 维度完整"
    selected_ra, ra_resolution = _resolve_selected_ra(project_dir, st, current_story)
    if selected_ra is None:
        return GateResult("G-RA-2", name, "blocker", True,
                          "无 RA 文档（依赖 G-RA-1 判定）",
                          details={**ra_resolution, "stub": True})

    latest = selected_ra
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
                          details={**ra_resolution, "missing": missing_dims, "file": latest.name})

    # RAModel 12 维检查（RA-G02）
    missing_ra = [k for k in RA_RAMODEL_KEYWORDS if k not in content]
    if missing_ra:
        return GateResult("G-RA-2", name, "blocker", False,
                          f"RAModel 12 维决策记录缺失：{missing_ra}",
                          f"补全 RA §0.5 的 RAModel 12 维（{missing_ra}）",
                          details={**ra_resolution, "missing_ramodel": missing_ra, "file": latest.name})

    return GateResult("G-RA-2", name, "blocker", True,
                      f"8 维度齐全 + RAModel 12 维完整（{latest.name}）",
                      details={**ra_resolution, "file": latest.name, "ramodel_dims": 12})


def check_ra_derivatives(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-RA-3 RA 衍生章节完整（SKILL.md G-RA 规则隐含 + RA-G08/RA-G09/RA-G12）。

    状态机类需求（命中 STATE_MACHINE_KEYWORDS）必须填满 5 个衍生章节；
    非状态机类需求允许"不适用 + 理由"。
    """
    name = "RA 衍生章节完整"
    selected_ra, ra_resolution = _resolve_selected_ra(project_dir, st, current_story)
    if selected_ra is None:
        return GateResult("G-RA-3", name, "blocker", True,
                          "无 RA 文档（依赖 G-RA-1 判定）",
                          details={**ra_resolution, "stub": True})

    latest = selected_ra
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
                              details={**ra_resolution, "missing": missing_sections, "state_machine": True,
                                       "file": latest.name})
        # 非状态机：允许缺失，但要求有"不适用"声明
        has_not_applicable = any(kw in content for kw in ["不适用", "不涉及", "无需衍生"])
        if not has_not_applicable:
            return GateResult("G-RA-3", name, "blocker", False,
                              f"非状态机需求缺失衍生章节且无'不适用'声明：{missing_sections}",
                              f"补全章节或显式标注'不适用 + 理由'",
                              details={**ra_resolution, "missing": missing_sections, "state_machine": False,
                                       "file": latest.name})

    return GateResult("G-RA-3", name, "blocker", True,
                      f"衍生章节完整（state_machine={is_state_machine}，{latest.name}）",
                      details={**ra_resolution, "file": latest.name, "state_machine": is_state_machine,
                               "missing": missing_sections})


def _locate_ra_scanner(master_source: Optional[Path]) -> Optional[Path]:
    """在母版找 ra_authenticity_scan.py（对标 _locate_authenticity_scanner）。"""
    return _locate_runtime_script(master_source, "ra_authenticity_scan.py")


def _validate_ra_scanner_finding(finding, index: int) -> Optional[str]:
    """Validate the field intersection emitted by all authoritative RA scanners."""
    prefix = f"findings[{index}]"
    if not isinstance(finding, dict):
        return f"{prefix} must be an object"

    severity = finding.get("severity")
    if severity not in {"BLOCKER", "WARN"}:
        return f"{prefix}.severity must be BLOCKER or WARN, got {severity!r}"

    for field in ("rule", "path", "message"):
        value = finding.get(field)
        if not isinstance(value, str) or not value.strip():
            return f"{prefix}.{field} must be a non-empty string"

    line = finding.get("line")
    if isinstance(line, bool) or not isinstance(line, int) or line < 0:
        return f"{prefix}.line must be a non-negative integer"

    for field in ("snippet", "file"):
        if field in finding and not isinstance(finding[field], str):
            return f"{prefix}.{field} must be a string when present"

    if "lineno" in finding:
        lineno = finding["lineno"]
        if isinstance(lineno, bool) or not isinstance(lineno, int) or lineno < 0:
            return f"{prefix}.lineno must be a non-negative integer when present"

    return None


def _validate_ra_scanner_result(result) -> tuple[Optional[dict], Optional[str], dict]:
    """Validate the scanner JSON and its three-state exit contract."""
    returncode = getattr(result, "returncode", None)
    stdout = getattr(result, "stdout", "") or ""
    stderr = getattr(result, "stderr", "") or ""
    details = {
        "scanner_returncode": returncode,
        "scanner_stderr": stderr[:200],
    }

    if not stdout.strip():
        return None, f"扫描器未输出 JSON（exit={returncode}）", details
    try:
        report = _json.loads(stdout)
    except _json.JSONDecodeError as e:
        return None, f"扫描器 JSON 输出无法解析: {e}", details
    if not isinstance(report, dict):
        return None, "扫描器 JSON 输出必须是对象", details

    status = report.get("status")
    details["scanner_status"] = status
    if status not in {"PASS", "FAIL", "ERROR"}:
        return None, f"扫描器 status 契约无效: {status!r}", details
    ra_files = report.get("raFiles")
    if isinstance(ra_files, bool) or not isinstance(ra_files, int) or ra_files < 0:
        return None, f"扫描器 raFiles 契约无效: {ra_files!r}", details

    if status == "ERROR":
        if ra_files != 0:
            return None, "扫描器 ERROR 契约无效: raFiles must be 0", details
        error = report.get("error")
        if not isinstance(error, dict):
            return None, "扫描器 ERROR 契约无效: error must be an object", details
        for field in ("code", "message"):
            value = error.get(field)
            if not isinstance(value, str) or not value.strip():
                return None, (
                    f"扫描器 ERROR 契约无效: error.{field} must be a non-empty string"
                ), details
        findings = report.get("findings")
        if findings is not None:
            if not isinstance(findings, list):
                return None, "扫描器 ERROR 契约无效: findings must be an array when present", details
            for index, finding in enumerate(findings):
                finding_error = _validate_ra_scanner_finding(finding, index)
                if finding_error:
                    return None, f"扫描器 ERROR 契约无效: {finding_error}", details
        details["scanner_error_code"] = error["code"]
        return None, (
            f"扫描器返回 ERROR 状态: {error['code']}: {error['message']}"
        ), details

    findings = report.get("findings")
    if not isinstance(findings, list):
        return None, "扫描器 findings 契约无效（必须是数组）", details
    for index, finding in enumerate(findings):
        finding_error = _validate_ra_scanner_finding(finding, index)
        if finding_error:
            return None, f"扫描器 findings 契约无效: {finding_error}", details
    expected_returncode = 0 if status == "PASS" else 1
    if returncode != expected_returncode:
        return None, (
            "扫描器退出码与状态不一致："
            f"exit={returncode}, status={status}, expected={expected_returncode}"
        ), details
    return report, None, details


def check_ra_authenticity(project_dir: Path, st: dict, current_story: str,
                          master_source: Optional[Path] = None) -> GateResult:
    """G-RA-4 RA 真实性扫描通过（SKILL.md G-RA 规则 3/4 自动化部分）。

    调 ra_authenticity_scan.py 跑 8 类禁止检查（对标 G-09 调 test_authenticity_scan.py）。
    BLOCKER=0 → pass。
    """
    name = "RA 真实性扫描通过"
    phase = st.get("phase", "initialized")

    scanner = _locate_ra_scanner(master_source)
    if scanner is None:
        return GateResult("G-RA-4", name, "blocker", True,
                          "未找到母版 ra_authenticity_scan.py（跳过）",
                          action="确认母版路径",
                          details={"scanned": False, "skipped": True, "stub": True})

    # 0 RA 文档 + pre-RA phase → stub
    selected_ra, ra_resolution = _resolve_selected_ra(project_dir, st, current_story)
    if selected_ra is None:
        return GateResult("G-RA-4", name, "blocker", True,
                          "pre-RA 阶段无 RA 文档（stub 通过）",
                          details={**ra_resolution, "scanned": False, "stub": True, "phase": phase})

    # 跑扫描
    try:
        result = runtime_exec.run_command(
            [sys.executable, str(scanner), "--root", str(project_dir),
             "--file", str(selected_ra), "--format", "json"],
            capture_output=True, text=True, timeout=_GATE_SUBPROCESS_LIMIT_SECONDS,
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

    report, contract_error, contract_details = _validate_ra_scanner_result(result)
    if contract_error:
        return GateResult("G-RA-4", name, "blocker", False,
                          contract_error,
                          "检查 ra_authenticity_scan.py 的退出码与 JSON 输出契约",
                          details={**ra_resolution, **contract_details, "scanned": True})

    status = report.get("status", "UNKNOWN")
    ra_files_scanned = report.get("raFiles", 0)
    blockers = sum(1 for f in report.get("findings", []) if f.get("severity") == "BLOCKER")
    n_total = len(report.get("findings", []))

    if ra_files_scanned == 0:
        return GateResult("G-RA-4", name, "blocker", False,
                          "已选中权威 RA，但扫描器返回 raFiles=0",
                          "检查 --file 扫描范围与权威 RA 路径",
                          details={**ra_resolution, "scanned": True, "ra_files": 0})

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
                          details={**ra_resolution, "scanned": True, "ra_files": ra_files_scanned,
                                   "blockers": blockers, "total": n_total,
                                   "blocker_rules": blocker_rules,
                                   "sample_locations": location_hints})

    return GateResult("G-RA-4", name, "blocker", True,
                      f"RA 真实性扫描通过（{ra_files_scanned} 份 RA，0 BLOCKER，{n_total} WARN）",
                      details={**ra_resolution, "scanned": True, "ra_files": ra_files_scanned,
                               "blockers": 0, "total": n_total})


def _locate_flow_violation_scanner(master_source: Optional[Path]) -> Optional[Path]:
    """在母版找 flow_violation_scan.py（G-RA-FLOW-VIOLATION 运行时依赖，🆕 2026-06-27）。"""
    return _locate_runtime_script(master_source, "flow_violation_scan.py")


def check_ra_flow_violation(project_dir: Path, st: dict, current_story: str,
                             master_source: Optional[Path] = None) -> GateResult:
    """G-RA-FLOW-VIOLATION RA 流程违规审计通过（🆕 2026-06-27，建议书 §3.4）。

    调 flow_violation_scan.py 跑 8 条规则检查（R1 12 维 / R2 8 维度 / R3 缺口
    / R4 规模 / R5 路由决策 / R6 RA-G 闸 / R7 5 问自检 / R8 缺口闭环）。
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

    selected_ra, ra_resolution = _resolve_selected_ra(project_dir, st, current_story)
    if selected_ra is None:
        return GateResult(
            "G-RA-FLOW-VIOLATION", name, "blocker", True,
            "No authoritative RA is available for flow scan.",
            details={**ra_resolution, "scanned": False, "ra_files": 0, "stub": True},
        )

    try:
        result = runtime_exec.run_command(
            [sys.executable, str(scanner), "--root", str(project_dir),
             "--file", str(selected_ra), "--format", "json", "--strict"],
            capture_output=True, text=True, timeout=_GATE_SUBPROCESS_LIMIT_SECONDS, check=False,
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

    report, contract_error, contract_details = _validate_ra_scanner_result(result)
    if contract_error:
        return GateResult("G-RA-FLOW-VIOLATION", name, "blocker", False,
                          contract_error,
                          "检查 flow_violation_scan.py 的退出码与 JSON 输出契约",
                          details={**ra_resolution, **contract_details, "scanned": True})

    status = report.get("status", "UNKNOWN")
    ra_files_scanned = report.get("raFiles", 0)
    blockers = sum(1 for f in report.get("findings", []) if f.get("severity") == "BLOCKER")
    n_total = len(report.get("findings", []))

    if ra_files_scanned == 0:
        return GateResult("G-RA-FLOW-VIOLATION", name, "blocker", False,
                          "已选中权威 RA，但扫描器返回 raFiles=0",
                          "检查 --file 扫描范围与权威 RA 路径",
                          details={**ra_resolution, "scanned": True, "ra_files": 0})

    if status != "PASS" or blockers > 0:
        blocker_rules = sorted({f.get("rule") for f in report.get("findings", [])
                                if f.get("severity") == "BLOCKER"})
        return GateResult("G-RA-FLOW-VIOLATION", name, "blocker", False,
                          f"RA 流程违规审计发现 {blockers} 个 BLOCKER（共 {n_total} 项）：{blocker_rules}",
                          "修复 RA 文档中标 BLOCKER 的项（补 12 维 / 8 维度 / 5 问 / 缺口 / 规模 / RA-G 闸判定）",
                          details={**ra_resolution, "scanned": True, "ra_files": ra_files_scanned,
                                   "blockers": blockers, "total": n_total,
                                   "blocker_rules": blocker_rules})

    return GateResult("G-RA-FLOW-VIOLATION", name, "blocker", True,
                      f"RA 流程违规审计通过（{ra_files_scanned} 份 RA，0 BLOCKER，{n_total} WARN）",
                      details={**ra_resolution, "scanned": True, "ra_files": ra_files_scanned,
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

    scanner = _locate_ra_depth_scanner(master_source)
    if scanner is None:
        return GateResult("G-RA-5", name, "blocker", True,
                          "未找到母版 ra_depth_scan.py（跳过）",
                          action="确认母版路径",
                          details={"scanned": False, "skipped": True, "stub": True})

    # pre-RA phase 无 RA 文档 → stub
    selected_ra, ra_resolution = _resolve_selected_ra(project_dir, st, current_story)
    if selected_ra is None:
        return GateResult("G-RA-5", name, "blocker", True,
                          "pre-RA 阶段无 RA 文档（stub 通过）",
                          details={**ra_resolution, "scanned": False, "stub": True, "phase": phase})

    try:
        result = runtime_exec.run_command(
            [sys.executable, str(scanner), "--root", str(project_dir),
             "--file", str(selected_ra), "--format", "json", "--strict"],
            capture_output=True, text=True, timeout=_GATE_SUBPROCESS_LIMIT_SECONDS,
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

    report, contract_error, contract_details = _validate_ra_scanner_result(result)
    if contract_error:
        return GateResult("G-RA-5", name, "blocker", False,
                          contract_error,
                          "检查 ra_depth_scan.py 的退出码与 JSON 输出契约",
                          details={**ra_resolution, **contract_details, "scanned": True})

    status = report.get("status", "UNKNOWN")
    ra_files_scanned = report.get("raFiles", 0)
    blockers = sum(1 for f in report.get("findings", []) if f.get("severity") == "BLOCKER")
    n_total = len(report.get("findings", []))

    if ra_files_scanned == 0:
        return GateResult("G-RA-5", name, "blocker", False,
                          "已选中权威 RA，但扫描器返回 raFiles=0",
                          "检查 --file 扫描范围与权威 RA 路径",
                          details={**ra_resolution, "scanned": True, "ra_files": 0})

    if status != "PASS" or blockers > 0:
        blocker_rules = sorted({f.get("rule") for f in report.get("findings", [])
                                if f.get("severity") == "BLOCKER"})
        return GateResult("G-RA-5", name, "blocker", False,
                          f"RA 机械派生深度扫描发现 {blockers} 个 BLOCKER（共 {n_total} 项）：{blocker_rules}",
                          "修复 RA 文档中标 BLOCKER 的项（E.5/G.5/H.6/H.5 机械追问逐行可见 + 链接齐全 + 覆盖率真实 + 五问覆盖 + 业务模式六选一）",
                          details={**ra_resolution, "scanned": True, "ra_files": ra_files_scanned,
                                   "blockers": blockers, "total": n_total,
                                   "blocker_rules": blocker_rules})

    return GateResult("G-RA-5", name, "blocker", True,
                      f"RA 机械派生深度扫描通过（{ra_files_scanned} 份 RA，0 BLOCKER，{n_total} WARN）",
                      details={**ra_resolution, "scanned": True, "ra_files": ra_files_scanned,
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

    scanner = _locate_ra_implementation_scanner(master_source)
    if scanner is None:
        return GateResult("G-RA-6", name, "blocker", True,
                          "未找到母版 ra_implementation_scan.py（跳过）",
                          action="确认母版路径",
                          details={"scanned": False, "skipped": True, "stub": True})

    selected_ra, ra_resolution = _resolve_selected_ra(project_dir, st, current_story)
    if selected_ra is None:
        return GateResult("G-RA-6", name, "blocker", True,
                          "pre-RA 阶段无 RA 文档（stub 通过）",
                          details={**ra_resolution, "scanned": False, "stub": True, "phase": phase})

    try:
        result = runtime_exec.run_command(
            [sys.executable, str(scanner), "--root", str(project_dir),
             "--file", str(selected_ra), "--format", "json", "--strict"],
            capture_output=True, text=True, timeout=_GATE_SUBPROCESS_LIMIT_SECONDS,
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

    report, contract_error, contract_details = _validate_ra_scanner_result(result)
    if contract_error:
        return GateResult("G-RA-6", name, "blocker", False,
                          contract_error,
                          "检查 ra_implementation_scan.py 的退出码与 JSON 输出契约",
                          details={**ra_resolution, **contract_details, "scanned": True})

    status = report.get("status", "UNKNOWN")
    ra_files_scanned = report.get("raFiles", 0)
    blockers = sum(1 for f in report.get("findings", []) if f.get("severity") == "BLOCKER")
    n_total = len(report.get("findings", []))

    if ra_files_scanned == 0:
        return GateResult("G-RA-6", name, "blocker", False,
                          "已选中权威 RA，但扫描器返回 raFiles=0",
                          "检查 --file 扫描范围与权威 RA 路径",
                          details={**ra_resolution, "scanned": True, "ra_files": 0})

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
                          details={**ra_resolution, "scanned": True, "ra_files": ra_files_scanned,
                                   "blockers": blockers, "total": n_total,
                                   "blocker_rules": blocker_rules,
                                   "sample_locations": sample_locations})

    return GateResult("G-RA-6", name, "blocker", True,
                      f"RA 实现视角扫描通过（{ra_files_scanned} 份 RA，0 BLOCKER，{n_total} WARN）",
                      details={**ra_resolution, "scanned": True, "ra_files": ra_files_scanned,
                               "blockers": 0, "total": n_total})


# ─── G-14：CodingPlan-Story 一致性（建议书4 G-08-15）─────────────────────────
# ExecutionPlan 涉及的接口/DO/AC 必须与 Story 可对应；偏离时先更新 Story。
# 设计在 ④bis（CodingPlan 生成）-> ⑤ Coding 之间硬拦截。
# 🆕 v3.10.5 BUG6：AC ID 正则加负向边界 (?<![A-Za-z0-9])，防 MAC1 被子串匹配为 AC1。
# 保留无分隔 AC1 / 带分隔 AC-1 / AC_1 / AC100 全部匹配。
_AC_ID_RE = re.compile(r"(?<![A-Za-z0-9])AC[-_]?\d+")
# 🆕 v3.10.5 BUG5(G-14)：Story ID 模式，用于判定 CodingPlan 是否含任何 STORY-xxx ID。
_STORY_ID_RE = re.compile(r"STORY[-_]?\d+", re.IGNORECASE)


def _story_http_ac_ids(content: str) -> list[str]:
    """Read HTTP/interface ACs from structured Story verification tables."""
    lines = content.splitlines()
    http_acs: set[str] = set()
    index = 0
    while index + 1 < len(lines):
        header_line = lines[index].strip()
        separator_line = lines[index + 1].strip()
        if not (header_line.startswith("|") and separator_line.startswith("|")):
            index += 1
            continue
        headers = [cell.strip() for cell in header_line.strip("|").split("|")]
        separators = [cell.strip() for cell in separator_line.strip("|").split("|")]
        if len(headers) != len(separators) or not all(
            re.fullmatch(r":?-{3,}:?", cell) for cell in separators
        ):
            index += 1
            continue

        normalized_headers = [
            re.sub(r"[\s`_/-]+", "", header).casefold() for header in headers
        ]
        ac_col = next((i for i, header in enumerate(normalized_headers)
                       if header in {"ac", "acid", "验收标准", "验收标准id"}
                       or header.startswith("acid")), -1)
        boundary_col = next((i for i, header in enumerate(normalized_headers)
                             if "验证边界" in header or "测试边界" in header
                             or header == "boundary"), -1)
        level_col = next((i for i, header in enumerate(normalized_headers)
                          if "测试层级" in header or "验证层级" in header
                          or header == "testlevel"), -1)
        if ac_col < 0 or (boundary_col < 0 and level_col < 0):
            index += 2
            continue

        row = index + 2
        while row < len(lines) and lines[row].strip().startswith("|"):
            cells = [cell.strip() for cell in lines[row].strip().strip("|").split("|")]
            if len(cells) < len(headers):
                cells.extend([""] * (len(headers) - len(cells)))
            boundary = cells[boundary_col].casefold() if boundary_col >= 0 else ""
            level = cells[level_col].casefold() if level_col >= 0 else ""
            is_http = "http" in boundary or boundary.strip() == "rest"
            is_interface = level.strip() in {"接口", "接口测试", "api", "http", "rest"}
            if is_http or is_interface:
                http_acs.update(_AC_ID_RE.findall(cells[ac_col]))
            row += 1
        index = row
    return sorted(http_acs)


def check_g14(project_dir: Path, st: dict, current_story: str) -> GateResult:
    """G-14 CodingPlan-Story 一致性 - Plan 须引用 Story 且关键设计可对应"""
    name = "CodingPlan-Story 一致性"
    work_item = _work_item_from_state(st)
    entry_node = str(st.get("entryNode") or "").upper()
    standalone_micro = (
        st.get("scale") == "微"
        and entry_node not in {"STORY", "DR", "PRD"}
        and (not current_story or current_story == work_item)
    )
    if standalone_micro:
        return GateResult(
            "G-14", name, "blocker", True,
            "standalone 微任务无 Story，上游一致性检查不适用",
            details={"skipped": True, "alignmentMode": "standalone-micro",
                     "workItem": work_item},
        )
    if not current_story:
        return GateResult("G-14", name, "blocker", False, "state.currentStory 为空")

    plan = st.get("executionPlan") if isinstance(st.get("executionPlan"), dict) else {}
    if plan.get("goal") or plan.get("changedPaths") or plan.get("verification"):
        story_doc, story_resolution = _resolve_story_doc(project_dir, st, current_story)
        if story_doc is None or story_resolution.get("errorCode"):
            return GateResult(
                "G-14", name, "blocker", False,
                "Story 文档不存在或绑定无效，无法核对 executionPlan",
                _story_binding_action(st, current_story), details=story_resolution,
            )
        story_content = story_doc.read_text(encoding="utf-8")
        story_acs = set(_AC_ID_RE.findall(story_content))
        plan_content = _json.dumps(plan, ensure_ascii=False)
        plan_acs = set(_AC_ID_RE.findall(plan_content))
        missing_acs = sorted(story_acs - plan_acs)
        if missing_acs:
            return GateResult(
                "G-14", name, "blocker", False,
                f"executionPlan 未覆盖 Story AC: {missing_acs}",
                "在 executionPlan.verification 中补齐 AC 映射",
                details={**story_resolution, "missingAcs": missing_acs,
                         "planAcs": sorted(plan_acs)},
            )
        http_acs = _story_http_ac_ids(story_content)
        if http_acs:
            verification = plan.get("verification") if isinstance(plan.get("verification"), list) else []
            plan_boundaries = {
                str(item.get("acId") or item.get("ac") or "").strip():
                str(item.get("boundary") or "").strip().casefold()
                for item in verification if isinstance(item, dict)
            }
            mismatched = [ac_id for ac_id in http_acs if plan_boundaries.get(ac_id) != "http"]
            if mismatched:
                return GateResult(
                    "G-14", "ExecutionPlan-Story HTTP 边界一致性", "blocker", False,
                    f"Story 接口级 AC 未使用 HTTP verification: {mismatched}",
                    "把对应 executionPlan.verification.boundary 改为 http，并补齐双阶段契约",
                    details={**story_resolution, "reason": "http-ac-boundary-mismatch",
                             "httpAcs": http_acs, "mismatchedAcs": mismatched,
                             "planBoundaries": plan_boundaries},
                )
        return GateResult(
            "G-14", "ExecutionPlan-Story 一致性", "blocker", True,
            f"executionPlan 与 Story 一致（AC 对齐 {len(plan_acs)} 个）",
            details={**story_resolution, "profile": "compact",
                     "ac_ids_in_plan": sorted(plan_acs), "story_doc": str(story_doc)},
        )

    cp, resolution = _resolve_codingplan_doc(project_dir, st, current_story)
    if resolution.get("errorCode"):
        return GateResult("G-14", name, "blocker", False,
                          "CodingPlan legacy Story 候选不唯一，拒绝猜测",
                          "显式传入当前 Work Item 或清理重复 CodingPlan",
                          details=resolution)
    if cp is None:
        return GateResult("G-14", name, "blocker", False,
                          "CodingPlan 文档不存在，无法核对一致性",
                          f"先生成 {current_story}-CodingPlan.md",
                          details=resolution)

    # Story 文档须存在（G-02 范畴，此处复用同一 StoryName/bound-path 解析器）
    story_doc, story_resolution = _resolve_story_doc(project_dir, st, current_story)
    if story_resolution.get("errorCode"):
        return GateResult(
            "G-14", name, "blocker", False,
            f"Story 文档绑定无效: {story_resolution['errorCode']}",
            _story_binding_action(st, current_story),
            details={**resolution, **story_resolution},
        )
    cp_content = cp.read_text(encoding="utf-8")

    issues: list[str] = []

    # 1. CodingPlan 须含 Story 文档引用（路径或 STORY-ID），且引用文件存在
    # 🆕 v3.10.5 BUG5(G-14)：原 current_story in cp or "Story" in cp，通用词 "Story"
    # 子串即可绕过 Story ID 引用检查。改为：含任何 STORY-xxx ID 时必须命中 current_story；
    # 完全无 ID 时才允许边界词 \bStory\b 通过（兼容早期草稿）。
    has_real_story_id = bool(_STORY_ID_RE.search(cp_content))
    has_story_ref = (current_story in cp_content) or (
        not has_real_story_id and re.search(r"\bStory\b", cp_content) is not None)
    if not has_story_ref:
        issues.append(f"CodingPlan 未引用 Story 文档（无 '{current_story}' 或 'Story' 字样）")
    elif story_doc is None:
        issues.append(f"CodingPlan 引用的 Story 文档不存在：{current_story}.md")

    # 2. AC ID 对齐：CodingPlan 测试章节须覆盖 Story 的 AC（至少出现 AC 编号）
    ac_ids_in_cp = set(_AC_ID_RE.findall(cp_content))
    if not ac_ids_in_cp:
        # 无 AC 引用可能是微任务场景；仅当 Story 文档含 AC 而 CodingPlan 无 -> issue
        if story_doc is not None:
            story_content = story_doc.read_text(encoding="utf-8")
            story_acs = set(_AC_ID_RE.findall(story_content))
            if story_acs and not (story_acs & ac_ids_in_cp):
                issues.append(f"Story 含 AC {sorted(story_acs)} 但 CodingPlan 测试章节未对齐任何 AC ID")

    # 3. 偏离 Story 设计必须先回写 Story；Proposal 已退役。
    if "偏离声明" in cp_content or "偏离" in cp_content:
        issues.append("CodingPlan 含偏离声明；请先更新 Story 当前契约，禁止用 Proposal 旁路")

    if issues:
        return GateResult("G-14", name, "blocker", False,
                          f"CodingPlan-Story 一致性未通过（{len(issues)} 项）：{'; '.join(issues)}",
                          f"修复 {current_story}-CodingPlan.md 使其与 Story 一致；偏离时先更新 Story",
                          details={**resolution, **story_resolution,
                                   "issues": issues, "ac_ids_in_cp": sorted(ac_ids_in_cp),
                                   "story_doc_exists": story_doc is not None})

    return GateResult("G-14", name, "blocker", True,
                      f"CodingPlan-Story 一致性通过（AC 对齐 {len(ac_ids_in_cp)} 个，Story 引用存在）",
                      details={**resolution, **story_resolution,
                               "ac_ids_in_cp": sorted(ac_ids_in_cp),
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
    plan = st.get("executionPlan") if isinstance(st.get("executionPlan"), dict) else {}
    if plan.get("goal") or plan.get("changedPaths") or plan.get("verification"):
        source_reads = [str(item).strip().strip("`") for item in plan.get("sourceReads") or []]
        missing = [item for item in source_reads if not (project_dir / item).is_file()]
        if missing:
            return GateResult(
                "G-CODEPLAN-SRC", "ExecutionPlan 源码核对", "blocker", False,
                f"executionPlan.sourceReads 含不存在路径: {missing[:5]}",
                "修正 sourceReads；禁止伪造源码核对证据",
                details={"profile": "compact", "missing_read_files": missing},
            )
        return GateResult(
            "G-CODEPLAN-SRC", "ExecutionPlan 源码核对", "blocker", True,
            f"executionPlan 源码核对通过（{len(source_reads)} 个路径）",
            details={"profile": "compact", "read_files": source_reads,
                     "skipped": not source_reads},
        )
    cp, resolution = _resolve_codingplan_doc(project_dir, st, current_story)
    if resolution.get("errorCode"):
        return GateResult("G-CODEPLAN-SRC", name, "blocker", False,
                          "CodingPlan legacy Story 候选不唯一，拒绝猜测",
                          "显式传入当前 Work Item 或清理重复 CodingPlan",
                          details=resolution)
    if cp is None:
        return GateResult("G-CODEPLAN-SRC", name, "blocker", False,
                          "CodingPlan 文档不存在，无法核对源码",
                          "先生成 Work Item CodingPlan",
                          details=resolution)

    content = cp.read_text(encoding="utf-8")

    # 定位"关键类骨架"章节（§2 / 章节 2 / 关键类骨架）
    skeleton_section = _extract_skeleton_section(content)

    if skeleton_section is None:
        # 无类骨架章节（微任务可能无）→ 跳过（不阻断）
        return GateResult("G-CODEPLAN-SRC", name, "blocker", True,
                          "CodingPlan 无关键类骨架章节（微任务场景，跳过源码核对）",
                          details={**resolution, "skipped": True,
                                   "profile": "micro" if st.get("scale") == "微" else "full",
                                   "reason": "no skeleton section", "file": str(cp)})

    read_marks = _SRC_READ_RE.findall(skeleton_section)
    pending_marks = _SRC_PENDING_RE.findall(skeleton_section)

    # 校验已读源码标记的文件是否真实存在（防伪造标记）
    missing_read_files = []
    for ref_path in read_marks:
        ref_clean = ref_path.strip().strip("`").strip()
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
                          details={**resolution, "n_read": 0, "n_pending": 0})

    if n_pending > 0:
        return GateResult("G-CODEPLAN-SRC", name, "blocker", False,
                          f"CodingPlan 有 {n_pending} 个待核实源码标记未闭环（待核实清单非空禁止进 Coding）：{pending_marks}",
                          f"补读现有同类源码后，把【待核实源码】改为【已读源码：{'}路径{'}】",
                          details={**resolution, "n_read": n_read, "n_pending": n_pending,
                                   "pending": pending_marks})

    if missing_read_files:
        return GateResult("G-CODEPLAN-SRC", name, "blocker", False,
                          f"CodingPlan 标注已读但源码文件不存在（{len(missing_read_files)} 个）：{missing_read_files[:5]}",
                          "核对路径或改为【待核实源码】",
                          details={**resolution, "n_read": n_read, "n_pending": n_pending,
                                   "missing_read_files": missing_read_files})

    return GateResult("G-CODEPLAN-SRC", name, "blocker", True,
                      f"源码核对通过（{n_read} 个已读标记，0 待核实，文件均存在）",
                      details={**resolution, "n_read": n_read, "n_pending": 0,
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
            cwd=project_dir, capture_output=True, text=True, timeout=_GIT_INSPECTION_LIMIT_SECONDS,
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

_DOCUMENT_STORAGE_SOURCE_ARTIFACTS = frozenset({
    "skills/cross-cutting/document-storage-skill.md",
    "skill-fallbacks/skills/cross-cutting/document-storage-skill.full.md",
})
_DOCUMENT_STORAGE_RUNTIME_ARTIFACTS = frozenset({
    "runtime/skills/cross-cutting/document-storage-skill/fallback/skill.full.md",
})


def _is_document_storage_skill_artifact(md_path: Path, scan_root: Path) -> bool:
    """Return true only for canonical document-storage source/runtime artifacts."""
    try:
        rel = md_path.relative_to(scan_root).as_posix().casefold()
    except ValueError:
        return False

    allowed = _DOCUMENT_STORAGE_SOURCE_ARTIFACTS
    if scan_root.name.casefold() != "source":
        allowed = allowed | _DOCUMENT_STORAGE_RUNTIME_ARTIFACTS
    return rel in allowed


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

    # Installed/compiled packages contain generated runtime compact views. They
    # repeat source text for routing/navigation and are not authoritative input
    # documents. Exclude only that generated subtree; keep scanning the package
    # skills/, skill-fallbacks/, standards/, templates/, and other real inputs.
    is_compiled_package = (
        source_dir == master_source
        and (master_source / "runtime" / "manifest.json").is_file()
    )

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
        if is_compiled_package and rel_str.startswith("runtime/"):
            continue
        if _is_document_storage_skill_artifact(md_path, source_dir):
            continue  # SSOT source and compiler-derived fallbacks are equivalent artifacts
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
    # Project-side G-PATH scope is limited to declared memory inputs.  Drafts
    # are review/process artifacts, not canonical project memory documents.
    memory_dir = ae_sdd_dir / "memory"
    if memory_dir.is_dir():
        project_scan_targets.extend(memory_dir.rglob("*.md"))
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
        # Project-side memory is outside the master source scan root.  Apply
        # the same strict layout matcher without restoring the old basename
        # exemption; project copies must still be scanned.
        if _is_document_storage_skill_artifact(md_path, project_dir):
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

    校验 review 节点切相前，Review Batch v2 已达到风险策略退出条件；旧 state 走兼容校验。

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
                      "跑 `ae-sdd review-loop collect` 推进有效 batch；平台失败只重试失败角色，预算耗尽进入 STALLED",
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
    # G-REVIEW-LOOP 负责 Review Batch 风险策略退出，G-09B 负责启动与独立 session
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
        "pre_phases": {"initialized", "route-selected", "requirement-analyzed", "ra-generated"},
        "passed_phases": {"story-generated", "story-reviewed",
                          "testcase-generated", "testcase-reviewed",
                          "task-generated", "task-reviewed",
                          "coding-process", "coding", "test-running",
                          "code-reviewed", "completed"},
        "required": ["constraints", "assets", "RA"],
    },
    "G-STORY-CTX": {
        "name": "Story 上下文加载",
        "scales": {"大", "中", "小", "微"},
        "pre_phases": {"initialized", "route-selected", "requirement-analyzed", "ra-generated", "dr-generated"},
        "passed_phases": {"story-reviewed", "testcase-generated", "testcase-reviewed",
                          "task-generated", "task-reviewed",
                          "coding-process", "coding", "test-running",
                          "code-reviewed", "completed"},
        # 🆕 v3.9.3: dependsStory + sourceTrace 覆盖 SSOT §3 C 类与 §4 来源追溯
        # 🆕 v3.9.20: standardsRef 升级为真"已引用"门禁（查产物证据，不查行为）
        #              + scales 扩到 {大,中,小,微}，取消小/微豁免（小/微走轻量阈值）
        "required": ["constraints", "assets", "RA",
                     "dependsStory", "sourceTrace", "standardsRef",
                     "outputBoundary"],
    },
    "G-TESTCASE-CTX": {
        "name": "TestCase 上下文加载",
        "scales": {"大", "中", "小"},
        "pre_phases": {"initialized", "route-selected", "requirement-analyzed", "ra-generated", "dr-generated",
                       "story-generated", "story-reviewed"},
        "passed_phases": {"task-generated", "task-reviewed",
                          "coding-process", "coding", "test-running",
                          "code-reviewed", "completed"},
        "required": ["constraints", "assets", "Story"],
    },
    "G-TASK-CTX": {
        "name": "Task 上下文加载",
        "scales": {"大", "中", "小", "微"},
        "pre_phases": {"initialized", "route-selected", "requirement-analyzed", "ra-generated", "dr-generated",
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


_STORY_FORBIDDEN_HEADINGS = (
    "变更历史", "CHANGELOG", "生成过程", "撰写过程", "执行过程",
    "门禁记录", "门禁结果", "REVIEW 记录", "REVIEW 过程", "评审过程",
    "来源追溯报告", "AGENT 执行记录", "DR 文档", "DR 正文", "DR 草稿",
)
_STORY_FORBIDDEN_ARTIFACT_REFS = (
    "STORYGENERATEPLAN", "STORY_SOURCE_TRACE", "STORY_WRITER_REPORT",
    "DR_SUPPLEMENT",
)


def _check_story_output_boundary(project_dir: Path, current_story: str) -> tuple[bool, str]:
    """Story 正文只承载当前契约，不承载生成过程或旁路文档。"""
    if not current_story:
        return False, "state.currentStory 为空"

    story_doc = paths.find_doc(project_dir, current_story, ".md")
    if story_doc is None:
        return False, f"未找到 Story 文档 {current_story}"

    try:
        content = story_doc.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as exc:
        return False, f"Story 文档不可读：{exc}"

    violations: list[str] = []
    for line in content.splitlines():
        heading = re.match(r"^\s{0,3}#{1,6}\s+(.+?)\s*$", line)
        if not heading:
            continue
        normalized = heading.group(1).upper()
        if any(keyword.upper() in normalized for keyword in _STORY_FORBIDDEN_HEADINGS):
            violations.append(f"过程型章节：{heading.group(1)}")

    upper_content = content.upper()
    for artifact in _STORY_FORBIDDEN_ARTIFACT_REFS:
        if artifact in upper_content:
            violations.append(f"过程/旁路产物引用：{artifact}")

    if violations:
        return False, "；".join(dict.fromkeys(violations))
    return True, ""


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
        required_keys = list(spec["required"])
    selected_design = (st.get("routeDecision") or {}).get("selectedDesign")
    if gate_id == "G-STORY-CTX" and (
        selected_design == "DR" or phase in {"dr-generated", "story-generated"}
    ):
        required_keys.insert(3, "DR")

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
            design = _g13_design_root(project_dir)
            drs = _iter_dr_docs(design)
            ok = bool(drs)
            status["DR"] = ok
            if not ok:
                missing.append("DR 文档")
                missing_hints.append(
                    "Story 链路只读 DR；停止并向用户报告缺少 DR，"
                    "不得在 Story 任务中运行 dr-generate-skill"
                )
        elif key == "Story":
            if not current_story:
                status["Story"] = False
                missing.append("Story 文档（state.currentStory 为空）")
                missing_hints.append(
                    "ae-sdd state write --phase story-generated --story STORY-XXX；"
                    "或由主流程监管器按 classify.spec_strategy 自动派 story-generate 子流程生成"
                )
            else:
                story_doc = paths.find_doc(project_dir, current_story, ".md")
                ok = story_doc is not None
                status["Story"] = ok
                if not ok:
                    missing.append(f"Story 文档（{current_story}.md）")
                    missing_hints.append(
                        f"跑 story-generate-skill 生成 {current_story}；"
                        "或由主流程监管器按 classify.spec_strategy 自动派 story-generate 子流程生成"
                    )
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
        elif key == "outputBoundary":
            ok, detail = _check_story_output_boundary(project_dir, current_story)
            status["outputBoundary"] = ok
            if not ok:
                missing.append(f"Story 输出边界：{detail}")
                missing_hints.append(
                    "删除 Story 中的生成/评审/门禁过程、CHANGELOG/变更历史、"
                    "DR 正文/草稿及 Plan/SourceTrace/WriterReport 引用后重跑门禁"
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
    "G-08": check_g08, "G-HTTP-1": check_g_http_1, "G-09": check_g09,
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
              project_key: str, only: Optional[str] = None,
              work_item: str = "") -> list[GateResult]:
    """跑全部门禁（14 主门禁 + G-RA + G-CODE 等）；only 指定时只跑那一个"""
    results: list[GateResult] = []

    # 读 state（如果 .ae-sdd 存在）
    if ade_sdd and work_item:
        state_path = paths.find_work_item_state_path(ade_sdd, work_item)
        if state_path is None:
            return [GateResult(
                gate_id="G-WORKITEM",
                name="Work Item state resolution",
                severity="blocker",
                pass_=False,
                message=f"未找到显式 Work Item state: {work_item}",
            )]
        st = state_mod.read_state(state_path)
        st = dict(st)
        st["_resolvedWorkItem"] = work_item
    elif ade_sdd:
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
