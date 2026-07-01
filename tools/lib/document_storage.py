"""
document_storage.py - 文档存放 API 代码层实现（document-storage-skill §4 的 Python SSOT）。

🆕 v4.1（2026-06-27，路径治理修订）：本模块把 document-storage-skill.md §4 声明的
14 个动态定位 API 从"纯文档契约"落地为可调用 Python 函数。此前这些 API 零实现，
所有 SKILL 文档里的 `resolve_path(intent=...)` / `save_doc(...)` 调用 100% 靠 LLM
读文档手动模拟，无代码兜底。

职责边界：
  - 本模块 = 路径定位 + 文档 IO 的代码实现（"怎么定位/怎么存"）
  - document-storage-skill.md = 路径 SSOT 定义 + API 契约（"存哪里/怎么命名"）
  - paths.py = 基础路径原语（assets / state / 项目根定位）

智能等级声明：
  - P1 核心 API（get_git_path 等 5 个）：纯路径拼接，确定性
  - P2 中级 API（save_doc 等 5 个）：版本号自增 + 文件 IO，确定性
  - P3 复杂 API（choose_iteration 等 4 个）：采用【规则启发式】（关键词匹配 +
    时间衰减），够用于门禁校验与默认推断，但非 LLM 级语义智能。
"""
from __future__ import annotations

import re
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Optional

from . import paths


# ─── intent → 路径模板表（对齐 document-storage-skill §2.2 8 类流程目录）────────
# 每条 = (intent, 子目录模板, 是否带版本号, STORING 分类)
# 模板占位符：{docWorkspace} {projectKey} {workItem} {storyId} {docId} {major} {minor}
# 未带版本号的文档（设计类）原地更新；带版本号的（事件报告）每次写新版本。
_PATH_TEMPLATES: dict[str, tuple[str, bool, str]] = {
    "PRD":               ("{docWorkspace}/ae-sdd-doc/PRD/{docId}.md",                  False, "PRD"),
    "ISSUE":             ("{docWorkspace}/ae-sdd-doc/Issue/{docId}.md",                False, "Issue"),
    "RA":                ("{docWorkspace}/ae-sdd-doc/RA/{docId}.md",                   False, "RA"),
    "RA_GENERATE_PLAN":  ("{docWorkspace}/ae-sdd-doc/RA/{docId}-GeneratePlan-r{minor}.md", True, "RA"),
    "RA_IMPACT":         ("{docWorkspace}/ae-sdd-doc/RA/{docId}-Impact-r{minor}.md",    True, "RA"),
    "RA_REVERSE_ISSUES": ("{docWorkspace}/ae-sdd-doc/RA/{docId}-ReverseIssues.md",      False, "RA"),
    "DR":                ("{docWorkspace}/ae-sdd-doc/DR/{docId}.md",                   False, "DR"),
    "DR_SUPPLEMENT":     ("{docWorkspace}/ae-sdd-doc/DR/{docId}-Supplement.md",         False, "DR"),
    "STORY":             ("{docWorkspace}/ae-sdd-doc/Story/{docId}.md",                False, "Story"),
    "STORY_SUPPLEMENT":  ("{docWorkspace}/ae-sdd-doc/Story/{workItem}/{workItem}-Supplement.md", False, "Story"),
    "STORY_GENERATE_PLAN": ("{docWorkspace}/ae-sdd-doc/Story/{workItem}/{workItem}-GeneratePlan-r{minor}.md", True, "Story"),
    "STORY_WRITER_REPORT": ("{docWorkspace}/ae-sdd-doc/Story/{workItem}/{workItem}-WriterReport-r{minor}.md", True, "Story"),
    "TASK":              ("{docWorkspace}/ae-sdd-doc/Task/{workItem}/{docId}.md",       False, "Task"),
    "TASK_SUPPLEMENT":   ("{docWorkspace}/ae-sdd-doc/Task/{workItem}/{docId}-Supplement.md", False, "Task"),
    "TASK_WRITER_REPORT": ("{docWorkspace}/ae-sdd-doc/Task/{workItem}/{workItem}-TaskWriterReport-r{minor}.md", True, "Task"),
    "TASK_REVIEW":       ("{docWorkspace}/ae-sdd-doc/Task/{workItem}/{workItem}-TaskReview-r{minor}.md", True, "Task"),
    "TASK_IMPL_PLAN":    ("{docWorkspace}/ae-sdd-doc/Task/{workItem}/{docId}-ImplPlan.md", False, "Task"),
    "CODING_PLAN":       ("{docWorkspace}/ae-sdd-doc/Coding/{workItem}/{workItem}-CodingPlan.md", False, "Coding"),
    "CODING_REPORT":     ("{docWorkspace}/ae-sdd-doc/Coding/{workItem}/{workItem}-CodingReport-v{major}-r{minor}.md", True, "Coding"),
    "CODING_ISSUE_LOG":  ("{docWorkspace}/ae-sdd-doc/Coding/{workItem}/{workItem}-CodingIssueLog.md", False, "Coding"),
    "TESTCASE":          ("{docWorkspace}/ae-sdd-doc/Test/{workItem}/{workItem}-testcase.md",  False, "Test"),
    "TESTCASE_COMPLIANCE_REPORT": ("{docWorkspace}/ae-sdd-doc/Test/{workItem}/{workItem}-TestCaseCompliance-r{minor}.md", True, "Test"),
    "TESTCASE_REVIEW":   ("{docWorkspace}/ae-sdd-doc/Test/{workItem}/{workItem}-TestCaseReview-r{minor}.md", True, "Test"),
    "TEST_REPORT":       ("{docWorkspace}/ae-sdd-doc/Test/{workItem}/{workItem}-Report-v{major}-r{minor}.md", True, "Test"),
    "CODE_REVIEW":       ("{docWorkspace}/ae-sdd-doc/CR/{workItem}/{workItem}-CodeReview-v{major}-r{minor}.md", True, "CR"),
    "TRACE_MATRIX":      ("{docWorkspace}/ae-sdd-doc/Coding/{workItem}/{workItem}-追溯矩阵-v{major}-r{minor}.md", True, "Coding"),
    "STORY_REVIEW":      ("{docWorkspace}/ae-sdd-doc/CR/{workItem}/{workItem}-StoryReviewReport-r{minor}.md", True, "CR"),
    "REVIEW_UPDATEPLAN": ("{docWorkspace}/ae-sdd-doc/CR/{workItem}/{workItem}-StoryReviewUpdatePlan-r{minor}.md", True, "CR"),
    "REVIEW_COMPARE":    ("{docWorkspace}/ae-sdd-doc/CR/{workItem}/{workItem}-ReviewCompare-v{major}-to-v{minor}.md", True, "CR"),
    "TASK_SMALL":        ("{docWorkspace}/ae-sdd-doc/iterations/{iterDate}/Task/{docId}/",  False, "Task"),
    "PLAN_MICRO":        ("{docWorkspace}/ae-sdd-doc/iterations/{iterDate}/Coding/{docId}/", False, "Coding"),
    "PROPOSAL":          ("{docWorkspace}/ae-sdd-doc/CR/{workItem}/{workItem}-Proposal.md",  False, "CR"),
    "PROPOSAL_ARCHIVE":  ("{docWorkspace}/ae-sdd-doc/CR/{workItem}/archive/{docId}.md", False, "CR"),
    "ASSETS":            ("{docWorkspace}/.ae-sdd/assets/{projectKey}/{projectKey}.assets.md", False, "Assets"),
}


_VERSION_SUFFIX_RE = re.compile(
    r"(?:-v(?P<dash_v>\d+)-r(?P<dash_r>\d+)"
    r"|-v(?P<dot_v>\d+)\.(?P<dot_m>\d+)(?:-r(?P<dot_r>\d+))?"
    r"|-r(?P<only_r>\d+)"
    r"|-v(?P<from_v>\d+)-to-v(?P<to_v>\d+))$"
)


def _versionless_stem(stem: str) -> str:
    """Strip a supported version suffix from a document stem."""
    return _VERSION_SUFFIX_RE.sub("", stem)


# ─── 错误码（对齐 §4.11）────────────────────────────────────────────────────
class DocStorageError(Exception):
    """document_storage 统一异常，带错误码。"""
    def __init__(self, code: str, message: str):
        self.code = code
        super().__init__(f"[{code}] {message}")


# ─── 返回数据类 ────────────────────────────────────────────────────────────────
@dataclass
class ResolvedPath:
    """resolve_path() 返回值（对齐 §4.1 ResolvedPath 接口）。"""
    full_path: str
    dir_path: str
    file_name: str
    version_suffix: Optional[str]
    changelog_path: str
    iteration_dir: str
    scope: str  # 'project' | 'service'
    storing_index_update: dict


@dataclass
class SaveResult:
    """save_doc() 返回值（对齐 §4.3 SaveResult）。"""
    success: bool
    new_version: Optional[str]
    changelog_entry: Optional[str]
    full_path: str
    error: Optional[str] = None


@dataclass
class IterationChoice:
    """choose_iteration() 返回值（对齐 §4.5 IterationChoice）。"""
    date: str
    strength: str  # 'strong' | 'weak' | 'none'
    reasoning: str


# ═══════════════════════════════════════════════════════════════════════════════
# P1 核心 API（纯路径拼接，确定性）
# ═══════════════════════════════════════════════════════════════════════════════

def get_git_path(ade_sdd: Path, project_key: str) -> Optional[str]:
    """§4.2：返回项目根绝对路径（从 assets.md §1 gitPath 读取）。

    E001（无资产）/ E002（字段空）→ 返回 None（调用方按缺失处理）。
    """
    val = paths.read_asset_field(ade_sdd, project_key, "gitPath")
    return val if val else None


def get_service_root(ade_sdd: Path, project_key: str, service_name: str) -> Optional[str]:
    """§4.2：返回微服务根 = {gitPath}/{service_name}。

    E004（微服务名不在 §2 列表）由调用方按需校验；本函数仅拼接。
    """
    git_path = get_git_path(ade_sdd, project_key)
    if git_path is None:
        return None
    return str(Path(git_path) / service_name)


def get_constraints(ade_sdd: Path, project_key: str) -> dict:
    """§4.2：返回约束文档名 → 完整路径映射。

    约束目录定位：{gitPath}/constraints/ 或 docWorkspace/constraints/（二者其一）。
    取代 SKILL 内直接写死的 constraints/ 路径引用。
    """
    git_path = get_git_path(ade_sdd, project_key)
    doc_ws = paths.resolve_doc_workspace(ade_sdd, project_key)
    candidates = []
    if git_path:
        candidates.append(Path(git_path) / "constraints")
    if doc_ws:
        candidates.append(doc_ws / "constraints")
    result: dict[str, str] = {}
    for cdir in candidates:
        if cdir.is_dir():
            for f in sorted(cdir.glob("*.md")):
                result[f.stem] = str(f)
    return result


def get_assets(ade_sdd: Path, project_key: str) -> list:
    """§4.2：返回项目资产文件路径列表（总览 + 工程级子文件）。

    取代 SKILL 内直接写死的 assets/{projectKey}/ 路径。
    复用 paths.find_module_asset_files（v4.1 支持 line 分组发现）。
    """
    return paths.find_module_asset_files(ade_sdd, project_key)


def resolve_path(ade_sdd: Path, project_key: str, intent: str,
                 story_id: Optional[str] = None, doc_id: Optional[str] = None,
                 service_name: Optional[str] = None, task_name: Optional[str] = None,
                 version: Optional[dict] = None,
                 iteration_date: Optional[str] = None,
                 work_item_id: Optional[str] = None) -> ResolvedPath:
    """§4.1：核心 API，推导文档完整落地路径。

    步骤（对齐 §4.1 行为）：
      1. 读 assets §1 gitPath + docWorkspacePath
      2. 校验 gitPath 存在性（E003：不存在则抛错）
      3. 按 intent 选路径模板（_PATH_TEMPLATES）
      4. 替换占位符
      5. choose_iteration 判定迭代（未指定时）
      6. 拼接版本号
      7. 返回 ResolvedPath

    Raises:
        DocStorageError: E001（无资产）/ E003（gitPath 无效）/ E008（docWorkspace 无效）
                         / 未知 intent
    """
    # 1. 读 gitPath + docWorkspace
    git_path = get_git_path(ade_sdd, project_key)
    if git_path is None:
        raise DocStorageError("E001", f"项目资产不存在或无 gitPath 字段: {project_key}")
    # 2. 校验 gitPath 存在性（E003，落地前强制触发）
    if not Path(git_path).exists():
        raise DocStorageError("E003", f"gitPath 路径不存在: {git_path}")
    doc_ws = paths.resolve_doc_workspace(ade_sdd, project_key)
    if doc_ws is None:
        doc_ws = Path(git_path)
    # E008：docWorkspacePath 声明但无效
    declared = paths.read_asset_field(ade_sdd, project_key, "docWorkspacePath")
    if declared and not doc_ws.exists():
        raise DocStorageError("E008", f"docWorkspacePath 声明但路径不存在: {declared}")

    # 3. 选模板
    if intent not in _PATH_TEMPLATES:
        raise DocStorageError("E000", f"未知 intent: {intent}（已知: {sorted(_PATH_TEMPLATES)}）")
    template, has_version, category = _PATH_TEMPLATES[intent]

    # 4-5. 迭代日期（TASK_SMALL/PLAN_MICRO 需要）
    iter_date = iteration_date
    if "{iterDate}" in template and not iter_date:
        iter_date = choose_iteration(ade_sdd, project_key, story_id or doc_id or "").date

    # 6. 版本号
    major, minor = _normalize_version(version, has_version)
    version_suffix = f"{major}.{minor}" if has_version else ""

    # 4. 替换占位符
    # workItem 是独立编码任务隔离键：PRD / BUG / OPT / Story 均可。
    # 旧调用只传 story_id 时，workItem 回退到 story_id，保持路径兼容。
    effective_work_item = work_item_id or story_id or doc_id or task_name or ""

    # docId 回退链：显式 doc_id > task_name > workItem > story_id（Story/PRD/RA/DR 等单标识文档，
    # 其 doc-id 语义上 = story-id/prd-id/issue-id 等，调用方常只传 story_id/work_item_id）
    effective_doc_id = doc_id or task_name or story_id or ""
    if not effective_doc_id:
        effective_doc_id = effective_work_item
    full = template.format(
        docWorkspace=str(doc_ws).replace("\\", "/"),
        projectKey=project_key,
        workItem=effective_work_item,
        storyId=story_id or "",
        docId=effective_doc_id,
        major=major,
        minor=minor,
        iterDate=iter_date or datetime.now().strftime("%Y-%m-%d"),
    )
    p = Path(full.replace("/", "\\" if "\\" in str(doc_ws) else "/") if False else full)
    # 统一用 Path，跨平台
    full_path_obj = Path(full)

    scope = "service" if service_name else "project"
    changelog_path = str(full_path_obj.parent / f"{_versionless_stem(full_path_obj.stem)}-changelog.md")

    return ResolvedPath(
        full_path=str(full_path_obj),
        dir_path=str(full_path_obj.parent),
        file_name=full_path_obj.name,
        version_suffix=version_suffix or None,
        changelog_path=changelog_path,
        iteration_dir=str(Path(doc_ws) / "ae-sdd-doc" / "iterations" / (iter_date or "")),
        scope=scope,
        storing_index_update={"category": category, "docType": intent, "fullPath": str(full_path_obj)},
    )


# ═══════════════════════════════════════════════════════════════════════════════
# P2 中级 API（版本号自增 + 文件 IO，确定性）
# ═══════════════════════════════════════════════════════════════════════════════

def _normalize_version(version: Optional[dict | str], has_version: bool) -> tuple[int, int]:
    """Return (major, minor/r) for versioned document paths.

    Skill docs use both {major, minor} and {v, r}; tests and adapters may pass a
    compact string. Keep all forms compatible so save_doc callers do not need to
    know the internal field names.
    """
    if not has_version or version is None:
        return (1, 1)
    if isinstance(version, dict):
        major = version.get("major", version.get("v", 1))
        minor = version.get("minor", version.get("r", 1))
        return (int(major), int(minor))
    text = str(version).strip()
    compare = re.search(r"v?(\d+)-to-v?(\d+)", text, re.IGNORECASE)
    if compare:
        return (int(compare.group(1)), int(compare.group(2)))
    m = re.search(r"v?(\d+)(?:[.\-]r?(\d+))?", text, re.IGNORECASE)
    if not m:
        return (1, 1)
    return (int(m.group(1)), int(m.group(2) or 1))


def get_latest_version(doc_dir: Path, stem_prefix: str) -> Optional[tuple[int, int]]:
    """§4.6：返回 {doc_dir} 下 {stem_prefix} 最新版本号。

    支持两种版本号格式（2026-07-01 修复：原仅匹配点格式，漏匹配事件类报告 dash 格式）：
      - 点格式：-v1.0.md（设计类历史格式）→ group(1)=major, group(2)=minor
      - dash 格式：-v1-r2.md（事件类报告格式）→ group(1)=v, group(4)=r
      - r-only：-r2.md（Review 报告）→ (1, 2)
      - compare：-v1-to-v2.md（跨轮对比）→ (1, 2)

    Returns:
        (major, minor/r) 最大者；无版本文件返回 None。
    """
    if not doc_dir.is_dir():
        return None
    best: Optional[tuple[int, int]] = None
    for f in doc_dir.glob(f"{stem_prefix}*.md"):
        m = _VERSION_SUFFIX_RE.search(f.stem)
        if m:
            if m.group("dash_v") is not None:
                cur = (int(m.group("dash_v")), int(m.group("dash_r")))
            elif m.group("dot_v") is not None:
                cur = (int(m.group("dot_v")), int(m.group("dot_m")))
            elif m.group("only_r") is not None:
                cur = (1, int(m.group("only_r")))
            else:
                cur = (int(m.group("from_v")), int(m.group("to_v")))
            if best is None or cur > best:
                best = cur
    return best


def get_changelog(changelog_path: Path) -> list[str]:
    """§4.7：返回 ChangeLog 文件内容（行列表），不存在返回空。"""
    if not changelog_path.is_file():
        return []
    return changelog_path.read_text(encoding="utf-8", errors="replace").splitlines()


# ─── 🆕 2026-06-27 RA 前置检查（requirement-analysis-skill §第 0.5/一/一 bis/二/七/三 bis 衔接）──
# 触发：save_doc(intent="RA", ...) 或 ra-gate CLI 调用前
# 豁免：intent ∈ {BUG, CONFIG}（RA skill §第 -1 步双重豁免，不在 save_doc 走 RA 检查路径）
# 行为：检查通过 = 静默；不通过 = raise DocStorageError("G-RA-*", ...)
_RA_REQUIRED_SECTIONS = [
    "§0.5 RequirementAnalysisModel",  # 12 维决策
    "§0.6 需求风险预判",
    "§2 角色",     # 8 维度之一：角色分析
    "§3 场景",     # 场景分析
    "§4 流程",     # 业务流程（含状态机）
    "§5 数据",     # 数据要素
    "§6 规则",     # 业务规则与约束
    "§7 设计方向", # 设计方向论证
    "§8 AC",       # 验收标准雏形
    "§9 假设",     # 隐性假设与验证
    "§10 缺口",    # 缺口管理
    "§11 规模",    # 规模裁定
]

_RA_REQUIRED_GATES = ["RA-G01", "RA-G02", "RA-G03", "RA-G04",
                       "RA-G05", "RA-G06", "RA-G07", "RA-G08"]

_RA_RAGENERATEPLAN_PATTERN = re.compile(r"RAGeneratePlan", re.IGNORECASE)
_RA_5QUESTION_PATTERN = re.compile(r"5\s*问自检|5-question|5question", re.IGNORECASE)
_RA_GATE_RESULT_PATTERN = re.compile(r"RA-G\d+\s*[:：]\s*(?:PASS|✅|通过)", re.IGNORECASE)


def check_ra_prerequisites(content: str) -> None:
    """🆕 2026-06-27 RA 文档落地前置检查（G-RA-PLAN / G-RA-COMPLETE / G-RA-GATES 三道子门禁）。

    来自 `2026-06-27-RA多轮挖掘流程未执行-自我修订建议书.md` §3.3，
    对齐 `requirement-analysis-skill.md` §Plan-first / §第 0.5 步 / §第七步。

    校验项（任一不通过即 raise DocStorageError）：
      G-RA-PLAN     — content 必须包含 RAGeneratePlan 字样（Plan-first 硬前置）
      G-RA-COMPLETE — content 必须含 12 个核心章节锚（§0.5/§0.6/§2~§11）
      G-RA-5CHECK   — content 必须含 5 问自检字样
      G-RA-GATES    — content 必须含 RA-G01~RA-G08 至少 4 个 PASS 标记

    豁免：本函数不读 intent；调用方（save_doc）应只在 intent=="RA" 时调用本函数。
    BUG / CONFIG intent 不会进入本检查路径（save_doc 双重豁免）。
    """
    if not content:
        raise DocStorageError("G-RA-PLAN",
            "RA 文档 content 为空（必须先有 RAGeneratePlan，见 requirement-analysis-skill §Plan-first）")

    # G-RA-PLAN：必须含 RAGeneratePlan 字样
    if not _RA_RAGENERATEPLAN_PATTERN.search(content):
        raise DocStorageError("G-RA-PLAN",
            "RA 文档必须先有 RAGeneratePlan 附件（见 requirement-analysis-skill §Plan-first 原则）")

    # G-RA-COMPLETE：必须含 12 个核心章节锚
    missing = [s for s in _RA_REQUIRED_SECTIONS if s not in content]
    if missing:
        raise DocStorageError("G-RA-COMPLETE",
            f"RA 文档缺失以下必填章节：{missing}（见 requirement-analysis-skill §整体流程）")

    # G-RA-5CHECK：必须含 5 问自检字样
    if not _RA_5QUESTION_PATTERN.search(content):
        raise DocStorageError("G-RA-5CHECK",
            "RA 文档必须含 5 问自检记录（见 requirement-analysis-skill §第一步 bis，通过率 100%）")

    # G-RA-GATES：必须含至少 4 个 RA-G01~RA-G08 的 PASS 标记
    gate_passes = _RA_GATE_RESULT_PATTERN.findall(content)
    if len(gate_passes) < 4:
        raise DocStorageError("G-RA-GATES",
            f"RA 文档必须显式列出 RA-G01~RA-G08 闸判定结果（至少 4 个 PASS），当前仅 {len(gate_passes)} 个"
            f"（见 requirement-analysis-skill §第七步）")

    # 全部通过：静默返回
    return


def save_doc(ade_sdd: Path, project_key: str, intent: str, content: str,
             story_id: Optional[str] = None, doc_id: Optional[str] = None,
             version: Optional[dict] = None,
             changelog_note: Optional[str] = None,
             work_item_id: Optional[str] = None) -> SaveResult:
    """§4.3：统一文档保存入口（版本号自增 + ChangeLog + 目录创建 + .gitignore）。

    步骤（对齐 §4.3）：
      1. resolve_path 推导路径
      2. 🆕 2026-06-27 RA 类型强检查（intent=RA 时调用 check_ra_prerequisites）
         — BUG / CONFIG intent 双重豁免（RA skill §第 -1 步豁免规则）
      3. 若带版本号且未显式传 version → get_latest_version 自增（E007：重入必须递增）
      4. 写文件（旧版本保留）
      5. 追加 ChangeLog
      6. 首次写入 ae-sdd-doc/ 时维护 .gitignore（§7.3 承诺，2026-07-01 补齐）
      7. 返回 SaveResult
    """
    try:
        resolved = resolve_path(ade_sdd, project_key, intent,
                                story_id=story_id, doc_id=doc_id, version=version,
                                work_item_id=work_item_id)
    except DocStorageError as e:
        return SaveResult(False, None, None, "", error=str(e))

    # 🆕 2026-06-27 RA 类型强检查前置（双重豁免 BUG/CONFIG，详见 RA skill §第 -1 步）
    if intent == "RA":
        try:
            check_ra_prerequisites(content)
        except DocStorageError as e:
            return SaveResult(False, None, None, "", error=str(e))

    full_path = Path(resolved.full_path)
    full_path.parent.mkdir(parents=True, exist_ok=True)

    new_version = None
    if resolved.version_suffix:
        # 带版本号：未显式指定时自增（E007：重入必须递增）
        if version is None:
            stem = _versionless_stem(full_path.stem)
            latest = get_latest_version(full_path.parent, stem)
            if latest:
                new_version = f"{latest[0]}.{latest[1] + 1}"
            else:
                new_version = "1.1"
        else:
            new_version = resolved.version_suffix

    # 🆕 2026-07-01 修复：自增版本号后必须重新拼接路径，否则都写到初始版本路径（互相覆盖）
    # 仅在 version is None（自动自增）且新版本号 ≠ 初始版本号时重 resolve
    if resolved.version_suffix and version is None and new_version and new_version != resolved.version_suffix:
        _vparts = new_version.split(".")
        re_resolved = resolve_path(ade_sdd, project_key, intent,
                                   story_id=story_id, doc_id=doc_id,
                                   version={"major": int(_vparts[0]), "minor": int(_vparts[1])},
                                   work_item_id=work_item_id)
        full_path = Path(re_resolved.full_path)
        full_path.parent.mkdir(parents=True, exist_ok=True)
        resolved = re_resolved

    full_path.write_text(content, encoding="utf-8")

    changelog_entry = None
    if changelog_note:
        cl_path = Path(resolved.changelog_path)
        entry = f"- {datetime.now().strftime('%Y-%m-%d %H:%M')} v{new_version or 'N/A'}: {changelog_note}"
        with cl_path.open("a", encoding="utf-8") as f:
            f.write(entry + "\n")
        changelog_entry = entry

    # 🆕 2026-07-01 §7.3：首次写入 ae-sdd-doc/ 时维护 .gitignore（幂等）
    git_path = get_git_path(ade_sdd, project_key)
    if git_path:
        check_and_update_gitignore(Path(git_path), "# ae-sdd generated docs\nae-sdd-doc/")

    update_storing_index(ade_sdd, project_key, resolved.scope,
                         {"category": resolved.storing_index_update["category"],
                          "docType": intent, "fullPath": str(full_path)})

    return SaveResult(True, new_version, changelog_entry, str(full_path))


def finalize_doc(ade_sdd: Path, project_key: str, intent: str, file_path: str,
                 story_id: Optional[str] = None, doc_id: Optional[str] = None,
                 changelog_note: Optional[str] = None,
                 work_item_id: Optional[str] = None) -> SaveResult:
    """§4.3 补充：对已手写文件补版本号/ChangeLog/STORING（不改文件位置，不覆盖内容）。

    适用场景：未实现 intent（📝 标记）的文档，LLM 手写后用本函数补登记；
    或 LLM 已 Write 到最终路径但漏跑 save_doc 的后处理步骤。

    与 save_doc 的区别：
      - save_doc：resolve 推路径 + 写文件 + 后处理（全流程）
      - finalize_doc：**不写文件、不 resolve**（文件已由调用方写好），只跑后处理
        （ChangeLog + STORING + .gitignore），用已写文件的 resolve 结果登记。

    步骤：
      1. 校验 file_path 存在
      2. resolve_path 推路径（用于 STORING 登记的 category/docType）
      3. 追加 ChangeLog（同级目录，仅当传 changelog_note）
      4. 维护 .gitignore（首次写入）
      5. 更新 STORING.md 索引
      6. 返回 SaveResult（不覆盖文件内容）

    Raises:
        DocStorageError: file_path 不存在 / resolve_path 失败（E001/E003/E008/E000）
    """
    target = Path(file_path)
    if not target.is_file():
        raise DocStorageError("E009", f"finalize 目标文件不存在: {file_path}")

    # resolve 用于拿 STORING 登记所需的 category（不强制路径与已写文件一致，
    # 因为未实现 intent 的模板可能缺；已写文件路径以 file_path 为准）
    resolved = resolve_path(ade_sdd, project_key, intent,
                            story_id=story_id, doc_id=doc_id,
                            work_item_id=work_item_id)

    # 版本号（从文件名解析，若已带 -v{N}-r{M}）
    new_version = resolved.version_suffix

    changelog_entry = None
    if changelog_note:
        # ChangeLog 落在已写文件的同级目录（与 save_doc 一致）
        cl_path = target.parent / f"{_versionless_stem(target.stem)}-changelog.md"
        entry = f"- {datetime.now().strftime('%Y-%m-%d %H:%M')} v{new_version or 'N/A'}: {changelog_note}"
        with cl_path.open("a", encoding="utf-8") as f:
            f.write(entry + "\n")
        changelog_entry = entry

    # 维护 .gitignore（幂等）
    git_path = get_git_path(ade_sdd, project_key)
    if git_path:
        check_and_update_gitignore(Path(git_path), "# ae-sdd generated docs\nae-sdd-doc/")

    # STORING 登记用已写文件的实际路径（file_path），而非 resolved.full_path
    update_storing_index(ade_sdd, project_key, resolved.scope,
                         {"category": resolved.storing_index_update["category"],
                          "docType": intent, "fullPath": str(target)})

    return SaveResult(True, new_version, changelog_entry, str(target))


def update_storing_index(ade_sdd: Path, project_key: str, scope: str, entry: dict) -> None:
    """§4.4：更新 STORING.md 索引（单一项目级 ae-sdd-doc/STORING.md）。

    幂等：同 fullPath 不重复追加。
    注：scope 参数当前保留以备小任务旧路径分支（后续兼容增强），项目级统一写单一索引。
    """
    doc_ws = paths.resolve_doc_workspace(ade_sdd, project_key)
    if doc_ws is None:
        return
    storing = doc_ws / "ae-sdd-doc" / "STORING.md"
    line = f"| {entry.get('category', '')} | {entry.get('docType', '')} | {entry.get('fullPath', '')} |"
    existing = ""
    if storing.is_file():
        existing = storing.read_text(encoding="utf-8", errors="replace")
    if entry.get("fullPath", "") and entry["fullPath"] not in existing:
        storing.parent.mkdir(parents=True, exist_ok=True)
        with storing.open("a", encoding="utf-8") as f:
            f.write(line + "\n")


def check_and_update_gitignore(project_dir: Path, pattern: str) -> bool:
    """§4.8：检查 .gitignore 是否含 pattern，无则追加。返回是否新增。

    避免 ae-sdd-doc/iterations/ 等大目录污染 git。
    """
    gi = project_dir / ".gitignore"
    current = ""
    if gi.is_file():
        current = gi.read_text(encoding="utf-8", errors="replace")
    if pattern in current:
        return False
    with gi.open("a", encoding="utf-8") as f:
        f.write(("\n" if current and not current.endswith("\n") else "") + pattern + "\n")
    return True


# ═══════════════════════════════════════════════════════════════════════════════
# P3 复杂 API（规则启发式，非 LLM 级）
# ═══════════════════════════════════════════════════════════════════════════════

def check_business_coherence(doc_tags: list[str], iteration_dir: Path) -> str:
    """§4.5：业务关联性判定（启发式：关键词匹配）。

    扫 iteration_dir 下 .md，统计与 doc_tags 命中次数。
    Returns: 'strong'（≥3 命中）/ 'weak'（1-2）/ 'none'（0）。
    """
    if not iteration_dir.is_dir() or not doc_tags:
        return "none"
    hits = 0
    for md in iteration_dir.rglob("*.md"):
        try:
            text = md.read_text(encoding="utf-8", errors="replace").lower()
        except OSError:
            continue
        hits += sum(1 for t in doc_tags if t.lower() in text)
    if hits >= 3:
        return "strong"
    if hits >= 1:
        return "weak"
    return "none"


def check_logical_coherence(doc_tags: list[str], iteration_dir: Path) -> str:
    """§4.5：逻辑关联性判定（启发式：与 business 同算法，语义上关注技术标签）。

    本实现与 check_business_coherence 同算法（均基于关键词命中），
    调用方可对两类标签做不同标注后分别传入。
    """
    return check_business_coherence(doc_tags, iteration_dir)


def choose_iteration(ade_sdd: Path, project_key: str, doc_signature: str,
                     doc_tags: Optional[list[str]] = None,
                     today: Optional[str] = None) -> IterationChoice:
    """§4.5：迭代归属判定（启发式：关联性 + 时间衰减）。

    规则：
      1. 扫 docWorkspace/ae-sdd-doc/iterations/*/ 现有迭代
      2. 对每个迭代跑 check_business_coherence（用 doc_tags 或 doc_signature 拆词）
      3. strong > weak > none；同级取最近日期（时间衰减）
      4. 全部 none → 返回今日（E005 在纯启发式下降级为"新建迭代"而非强制问用户）

    Note: 真正的 LLM 级语义关联需独立设计；本实现够用于门禁校验与默认推断。
    """
    today = today or datetime.now().strftime("%Y-%m-%d")
    doc_ws = paths.resolve_doc_workspace(ade_sdd, project_key)
    if doc_ws is None:
        return IterationChoice(today, "none", "无 docWorkspace，默认今日")

    iterations_root = doc_ws / "ae-sdd-doc" / "iterations"
    tags = doc_tags or [w for w in re.split(r"[\s\-_]", doc_signature) if len(w) > 1]

    best: Optional[IterationChoice] = None
    best_rank = -1  # strong=2 weak=1 none=0
    best_date = ""

    if iterations_root.is_dir():
        for it_dir in sorted(iterations_root.iterdir(), reverse=True):
            if not it_dir.is_dir():
                continue
            strength = check_business_coherence(tags, it_dir)
            rank = {"strong": 2, "weak": 1, "none": 0}[strength]
            if rank > best_rank or (rank == best_rank and rank > 0):
                best_rank = rank
                best_date = it_dir.name
                best = IterationChoice(it_dir.name, strength,
                                       f"与迭代 {it_dir.name} {strength} 关联（{len(tags)} 标签匹配）")

    if best is not None and best_rank > 0:
        return best
    return IterationChoice(today, "none", f"无强关联迭代，默认新建今日迭代（E005 降级）")


def migrate_old_docs(project_dir: Path, mode: str = "dry-run") -> dict:
    """§4.9：旧路径迁移（design/ .ae-task/ .ae-plan/ .spec/ → ae-sdd-doc/）。

    Args:
        mode: 'dry-run'（默认，只报告）| 'execute'（实际移动）

    Returns:
        MigrationReport: { scanned, migrated, skipped, items: [{from, to, status}] }
    """
    old_roots = ["design", ".ae-task", ".ae-plan", ".spec"]
    new_base = project_dir / "ae-sdd-doc" / "iterations" / datetime.now().strftime("%Y-%m-%d")
    report = {"scanned": 0, "migrated": 0, "skipped": 0, "items": []}

    for old in old_roots:
        old_dir = project_dir / old
        if not old_dir.is_dir():
            continue
        for md in old_dir.rglob("*.md"):
            report["scanned"] += 1
            rel = md.relative_to(old_dir)
            target = new_base / old / rel
            item = {"from": str(md), "to": str(target), "status": "dry-run"}
            if mode == "execute":
                try:
                    target.parent.mkdir(parents=True, exist_ok=True)
                    md.rename(target)
                    item["status"] = "migrated"
                    report["migrated"] += 1
                except OSError as e:
                    item["status"] = f"error: {e}"
                    report["skipped"] += 1
            else:
                report["skipped"] += 1
            report["items"].append(item)

    return report
