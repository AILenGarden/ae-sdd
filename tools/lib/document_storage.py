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

import hashlib
import json
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
    "CODING_REPORT":     ("{docWorkspace}/ae-sdd-doc/Coding/{workItem}/{workItem}-CodingReport.md", False, "Coding"),
    "CODING_ISSUE_LOG":  ("{docWorkspace}/ae-sdd-doc/Coding/{workItem}/{workItem}-CodingIssueLog.md", False, "Coding"),
    "TESTCASE":          ("{docWorkspace}/ae-sdd-doc/Test/{workItem}/{workItem}-testcase.md",  False, "Test"),
    "TESTCASE_COMPLIANCE_REPORT": ("{docWorkspace}/ae-sdd-doc/Test/{workItem}/{workItem}-TestCaseCompliance-r{minor}.md", True, "Test"),
    "TESTCASE_REVIEW":   ("{docWorkspace}/ae-sdd-doc/Test/{workItem}/{workItem}-TestCaseReview-r{minor}.md", True, "Test"),
    "TEST_REPORT":       ("{docWorkspace}/ae-sdd-doc/Test/{workItem}/{workItem}-Report.md", False, "Test"),
    "CODE_REVIEW":       ("{docWorkspace}/ae-sdd-doc/CR/{workItem}/{workItem}-CodeReview.md", False, "CR"),
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


# Work Item scoped output intents. Explicit work_item_id is the canonical
# execution-artifact owner. story_id remains a design/reference relation and is
# only the legacy bucket when no explicit Work Item was supplied.
_STORY_SCOPED_INTENTS: frozenset[str] = frozenset({
    "STORY_SUPPLEMENT", "STORY_GENERATE_PLAN", "STORY_WRITER_REPORT",
    "TASK", "TASK_SUPPLEMENT", "TASK_WRITER_REPORT", "TASK_REVIEW", "TASK_IMPL_PLAN",
    "CODING_PLAN", "CODING_REPORT", "CODING_ISSUE_LOG",
    "TESTCASE", "TESTCASE_COMPLIANCE_REPORT", "TESTCASE_REVIEW", "TEST_REPORT",
    "CODE_REVIEW", "TRACE_MATRIX",
    "STORY_REVIEW", "REVIEW_UPDATEPLAN", "REVIEW_COMPARE",
    "PROPOSAL", "PROPOSAL_ARCHIVE",
})


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


class ScopeAmbiguousError(Exception):
    """Raised when a legacy Story fallback has more than one valid artifact."""

    code = "SCOPE_AMBIGUOUS"

    def __init__(self, candidates: list[Path]):
        self.candidates = candidates
        super().__init__(
            f"[{self.code}] legacy Story scope has {len(candidates)} candidates"
        )


class StoryDocumentNameInvalidError(DocStorageError):
    """Raised when StoryName is not a filename basename."""

    def __init__(self, story_name: str):
        super().__init__(
            "STORY_DOC_NAME_INVALID",
            f"StoryName must be a filename basename, got: {story_name!r}",
        )


class StoryDocumentAmbiguousError(DocStorageError):
    """Raised when one exact StoryName maps to multiple valid documents."""

    def __init__(self, candidates: list[Path]):
        self.candidates = tuple(str(path) for path in candidates)
        super().__init__(
            "STORY_DOC_AMBIGUOUS",
            f"exact StoryName matched {len(candidates)} valid documents",
        )


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


@dataclass(frozen=True)
class ScopedArtifactResolution:
    path: Optional[Path]
    scope_source: str
    candidates: tuple[Path, ...] = ()


@dataclass(frozen=True)
class StoryDocumentResolution:
    """A deterministic Story document lookup result."""

    path: Optional[Path]
    story_id: str
    story_name: str
    source: str
    candidates: tuple[Path, ...] = ()
    rejected: tuple[dict[str, str], ...] = ()


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


def _existing_unique_paths(candidates: list[Path]) -> list[Path]:
    seen: set[Path] = set()
    existing: list[Path] = []
    for item in candidates:
        try:
            resolved = item.expanduser().resolve()
        except OSError:
            resolved = item.expanduser()
        if resolved in seen:
            continue
        seen.add(resolved)
        if resolved.is_file():
            existing.append(resolved)
    return existing


def get_thinking_engine(ade_sdd: Path, project_key: str) -> dict:
    """Return the Coding thinking engine document reference and content.

    Resolution order:
      1. project/doc-workspace overrides;
      2. project gitPath overrides;
      3. ae-sdd packaged standard under source/ or installed runtime.

    The return shape is JSON-friendly because SKILL documents refer to this as
    a document-storage API contract rather than an internal Python object.
    """
    rel = Path("standards") / "thinking" / "be-coding-thinking-engine.md"
    candidates: list[Path] = []

    doc_ws = paths.resolve_doc_workspace(ade_sdd, project_key)
    if doc_ws:
        candidates.extend([doc_ws / rel, doc_ws / ".ae-sdd" / rel])

    git_path = get_git_path(ade_sdd, project_key)
    if git_path:
        git_root = Path(git_path)
        candidates.extend([git_root / rel, git_root / ".ae-sdd" / rel])

    master = paths.locate_master_source()
    if master:
        candidates.extend([master / rel, master / "source" / rel])

    package_root = Path(__file__).resolve().parents[2]
    candidates.extend([package_root / "source" / rel, package_root / rel])

    for path in _existing_unique_paths(candidates):
        content = path.read_text(encoding="utf-8", errors="replace")
        try:
            source = str(path.relative_to(package_root))
        except ValueError:
            source = str(path)
        return {
            "path": str(path),
            "source": source,
            "content": content,
            "sha256": hashlib.sha256(content.encode("utf-8")).hexdigest(),
        }
    return {}


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
    #
    # Explicit Work Item identity wins. This prevents an independent BUG/OPT
    # attached to a Story from overwriting or consuming the Story's main
    # Coding/Test/Review artifacts. Old callers that only provide story_id keep
    # the historical Story bucket.
    effective_work_item = work_item_id or story_id or doc_id or task_name or ""

    # docId 回退链：显式 doc_id > task_name > workItem > story_id（Story/PRD/RA/DR 等单标识文档，
    # 其 doc-id 语义上 = story-id/prd-id/issue-id 等，调用方常只传 story_id/work_item_id）
    effective_doc_id = doc_id or task_name or ""
    if not effective_doc_id and intent in _STORY_SCOPED_INTENTS:
        effective_doc_id = effective_work_item
    if not effective_doc_id:
        effective_doc_id = story_id or ""
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


_STORY_METADATA_ID_RE = re.compile(
    r"(?im)^\s*(?:[-*+]\s*|\|\s*)"
    r"(?:\*\*)?Story\s*ID(?:\*\*)?\s*(?:\||[：:])\s*"
    r"`?([A-Za-z0-9][A-Za-z0-9._-]*)`?"
)


def normalize_story_name(story_name: str) -> str:
    """Normalize a StoryName to a basename without the optional .md suffix."""
    raw = (story_name or "").strip()
    if (not raw or raw in {".", ".."} or ".." in raw
            or "/" in raw or "\\" in raw or "\x00" in raw
            or any(char in raw for char in "*?[]")):
        raise StoryDocumentNameInvalidError(story_name)
    if raw.lower().endswith(".md"):
        raw = raw[:-3]
    if not raw:
        raise StoryDocumentNameInvalidError(story_name)
    return raw


def _story_metadata_ids(path: Path) -> tuple[str, ...]:
    try:
        content = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return ()
    return tuple(dict.fromkeys(match.group(1).strip()
                               for match in _STORY_METADATA_ID_RE.finditer(content)))


def _story_candidate_rejection(path: Path, story_id: str) -> Optional[dict[str, str]]:
    metadata_ids = _story_metadata_ids(path)
    if not metadata_ids:
        return {
            "code": "STORY_DOC_ID_MISSING",
            "path": str(path),
            "expectedStoryId": story_id,
            "actualStoryId": "",
        }
    if story_id.casefold() not in {value.casefold() for value in metadata_ids}:
        return {
            "code": "STORY_DOC_ID_MISMATCH",
            "path": str(path),
            "expectedStoryId": story_id,
            "actualStoryId": ",".join(metadata_ids),
        }
    return None


def _exact_story_name_candidates(project_dir: Path, file_name: str) -> list[Path]:
    candidates: list[Path] = []
    for root in paths.doc_search_roots(Path(project_dir)):
        if not root.is_dir():
            continue
        direct = root / file_name
        if direct.is_file():
            candidates.append(direct)
        candidates.extend(candidate for candidate in root.rglob(file_name)
                          if candidate.is_file())
    seen: set[Path] = set()
    unique: list[Path] = []
    for candidate in candidates:
        try:
            resolved = candidate.resolve()
        except OSError:
            resolved = candidate
        if resolved in seen:
            continue
        seen.add(resolved)
        unique.append(resolved)
    return unique


def _story_path_is_within_search_roots(project_dir: Path, candidate: Path) -> bool:
    try:
        resolved_candidate = candidate.resolve()
    except OSError:
        resolved_candidate = candidate
    for root in paths.doc_search_roots(Path(project_dir)):
        try:
            resolved_candidate.relative_to(root.resolve())
            return True
        except (OSError, ValueError):
            continue
    return False


def _story_id_only_candidates(project_dir: Path, story_id: str) -> list[Path]:
    """Return exact Story-category candidates; never scan Task/Coding buckets."""
    file_name = f"{story_id}.md"
    candidates: list[Path] = []
    for root in paths.doc_search_roots(Path(project_dir)):
        for candidate in (
            paths.project_design_dir(root) / file_name,
            root / file_name,
            root / "ae-sdd-doc" / "Story" / file_name,
        ):
            if not candidate.is_file():
                continue
            try:
                resolved = candidate.resolve()
            except OSError:
                resolved = candidate
            if _story_path_is_within_search_roots(Path(project_dir), resolved):
                candidates.append(resolved)
    return list(dict.fromkeys(candidates))


def resolve_story_document(
    project_dir: Path,
    *,
    story_id: str,
    story_name: str = "",
    bound_path: Optional[str] = None,
) -> StoryDocumentResolution:
    """Resolve one Story document without fuzzy ID matching.

    Priority is an already-bound path, then an exact StoryName basename, then
    the legacy/canonical ID-only filename when no StoryName was supplied.
    Formal names are accepted only when document metadata declares story_id.
    """
    logical_id = (story_id or "").strip()
    if not logical_id:
        raise DocStorageError("STORY_ID_REQUIRED", "story_id is required")

    normalized_name = normalize_story_name(story_name) if story_name else ""

    if bound_path:
        candidate = Path(bound_path).expanduser()
        if not candidate.is_absolute():
            candidate = Path(project_dir) / candidate
        try:
            candidate = candidate.resolve()
        except OSError:
            pass
        if candidate.is_file():
            if not _story_path_is_within_search_roots(Path(project_dir), candidate):
                rejection = {
                    "code": "STORY_DOC_OUTSIDE_ROOTS",
                    "path": str(candidate),
                    "expectedStoryId": logical_id,
                    "actualStoryId": "",
                }
                return StoryDocumentResolution(
                    None, logical_id, normalized_name or candidate.stem, "none",
                    candidates=(candidate,), rejected=(rejection,),
                )
            if (normalized_name
                    and candidate.name.casefold() != f"{normalized_name}.md".casefold()):
                rejection = {
                    "code": "STORY_DOC_NAME_MISMATCH",
                    "path": str(candidate),
                    "expectedStoryId": logical_id,
                    "actualStoryId": "",
                }
                return StoryDocumentResolution(
                    None, logical_id, normalized_name, "none",
                    candidates=(candidate,), rejected=(rejection,),
                )
            rejection = _story_candidate_rejection(candidate, logical_id)
            if rejection:
                return StoryDocumentResolution(
                    None, logical_id, normalized_name or candidate.stem, "none",
                    candidates=(candidate,), rejected=(rejection,),
                )
            return StoryDocumentResolution(
                candidate, logical_id, normalized_name or candidate.stem,
                "bound-path", candidates=(candidate,),
            )

    if normalized_name:
        candidates = _exact_story_name_candidates(
            Path(project_dir), f"{normalized_name}.md"
        )
        valid: list[Path] = []
        rejected: list[dict[str, str]] = []
        for candidate in candidates:
            if not _story_path_is_within_search_roots(Path(project_dir), candidate):
                rejected.append({
                    "code": "STORY_DOC_OUTSIDE_ROOTS",
                    "path": str(candidate),
                    "expectedStoryId": logical_id,
                    "actualStoryId": "",
                })
                continue
            rejection = _story_candidate_rejection(candidate, logical_id)
            if rejection:
                rejected.append(rejection)
            else:
                valid.append(candidate)
        if len(valid) > 1:
            raise StoryDocumentAmbiguousError(valid)
        if valid:
            return StoryDocumentResolution(
                valid[0], logical_id, normalized_name, "story-name",
                candidates=tuple(candidates), rejected=tuple(rejected),
            )
        return StoryDocumentResolution(
            None, logical_id, normalized_name, "none",
            candidates=tuple(candidates), rejected=tuple(rejected),
        )

    canonical_candidates = _story_id_only_candidates(Path(project_dir), logical_id)
    if len(canonical_candidates) > 1:
        raise StoryDocumentAmbiguousError(canonical_candidates)
    if canonical_candidates:
        canonical = canonical_candidates[0]
        return StoryDocumentResolution(
            canonical, logical_id, logical_id, "canonical-id",
            candidates=tuple(canonical_candidates),
        )
    return StoryDocumentResolution(None, logical_id, logical_id, "none")


def resolve_scoped_artifact(
    project_dir: Path,
    *,
    category: str,
    work_item_id: str,
    story_id: str,
    suffixes: list[str],
) -> ScopedArtifactResolution:
    """Resolve a Coding/Test/CR artifact without crossing Work Item identity.

    The explicit Work Item bucket is authoritative. A Story bucket is consulted
    only when no Work Item artifact exists, and that compatibility fallback must
    contain exactly one candidate.
    """

    roots = paths.doc_search_roots(Path(project_dir))

    def collect(identity: str) -> list[Path]:
        if not identity:
            return []
        candidates: list[Path] = []
        for root in roots:
            doc_root = root / "ae-sdd-doc"
            direct_dir = doc_root / category / identity
            for suffix in suffixes:
                pattern = f"{identity}{suffix}"
                candidates.extend(sorted(direct_dir.glob(pattern)))
                if doc_root.is_dir():
                    candidates.extend(sorted(doc_root.rglob(pattern)))
                candidates.extend(
                    [
                        paths.project_design_dir(root) / f"{identity}{suffix}",
                        root / f"{identity}{suffix}",
                    ]
                )
        seen: set[str] = set()
        result: list[Path] = []
        for candidate in candidates:
            if not candidate.is_file():
                continue
            key = str(candidate.resolve())
            if key in seen:
                continue
            seen.add(key)
            result.append(candidate)
        return result

    canonical = collect(work_item_id)
    if canonical:
        # Work Item artifacts are authoritative. Prefer the canonical direct
        # unversioned path, then deterministic lexical order for older layouts.
        direct_root_parts = ("ae-sdd-doc", category, work_item_id)

        def canonical_rank(path: Path) -> tuple[int, int, str]:
            normalized = path.as_posix()
            is_direct = all(part in path.parts for part in direct_root_parts)
            is_unversioned = "-v" not in path.stem and "-r" not in path.stem
            return (0 if is_direct else 1, 0 if is_unversioned else 1, normalized)

        selected = sorted(canonical, key=canonical_rank)[0]
        return ScopedArtifactResolution(
            path=selected,
            scope_source="work-item",
            candidates=tuple(canonical),
        )

    if not story_id or story_id == work_item_id:
        return ScopedArtifactResolution(None, "none")
    legacy = collect(story_id)
    if len(legacy) > 1:
        raise ScopeAmbiguousError(legacy)
    if legacy:
        return ScopedArtifactResolution(
            path=legacy[0],
            scope_source="legacy-story",
            candidates=tuple(legacy),
        )
    return ScopedArtifactResolution(None, "none")


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
# G-RA-COMPLETE 必填章节锚：每项为 (显示名, 标题行正则)。
# 正则匹配 markdown 标题行（^#+ 空格），与 ra-template.md 真实标题对齐
# （如 "## 2. 角色分析"）。旧实现用 "§2 角色" in content 松散子串匹配，
# 与真实标题格式（"2. 角色分析"）脱节，导致 12 项里 10 项连模板自身都过不了（C4 修复）。
_RA_REQUIRED_SECTIONS: list[tuple[str, re.Pattern]] = [
    ("§0.5 RequirementAnalysisModel", re.compile(r"^#{1,6}\s*0\.5\s*RequirementAnalysisModel", re.MULTILINE)),
    ("§0.6 需求风险预判", re.compile(r"^#{1,6}\s*0\.6\s*需求风险预判", re.MULTILINE)),
    ("§2 角色", re.compile(r"^#{1,6}\s*2\.?\s*角色", re.MULTILINE)),
    ("§3 场景", re.compile(r"^#{1,6}\s*3\.?\s*场景", re.MULTILINE)),
    ("§4 流程", re.compile(r"^#{1,6}\s*4\.?\s*(业务流程|流程)", re.MULTILINE)),
    ("§5 数据", re.compile(r"^#{1,6}\s*5\.?\s*数据", re.MULTILINE)),
    ("§6 规则", re.compile(r"^#{1,6}\s*6\.?\s*(业务规则|规则)", re.MULTILINE)),
    ("§7 设计方向", re.compile(r"^#{1,6}\s*7\.?\s*设计方向", re.MULTILINE)),
    ("§8 AC/验收标准", re.compile(r"^#{1,6}\s*8\.?\s*(验收标准|AC)", re.MULTILINE)),
    ("§9 假设", re.compile(r"^#{1,6}\s*9\.?\s*(隐性假设|假设)", re.MULTILINE)),
    ("§10 缺口", re.compile(r"^#{1,6}\s*10\.?\s*(缺口|未决)", re.MULTILINE)),
    ("§11 规模", re.compile(r"^#{1,6}\s*11\.?\s*规模", re.MULTILINE)),
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

    # G-RA-COMPLETE：必须含 12 个核心章节锚（标题行正则匹配，非松散子串）
    missing = [name for name, pattern in _RA_REQUIRED_SECTIONS if not pattern.search(content)]
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

    # 🆕 v3.10.1：禁用 ChangeLog 旁车文件生成。
    # 用户反馈：生成过程中改动不需要记录，文档原地更新即可，不要 {doc}-changelog.md 旁车文件。
    # changelog_note 参数保留兼容（CLI/调用方仍可传），但不再写入文件。
    changelog_entry = None

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

    # 🆕 v3.10.1：禁用 ChangeLog 旁车文件生成（同 save_doc）
    changelog_entry = None

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


# ─── P1 canonical document resolver ─────────────────────────────────────────
def _alias_registry_path(ade_sdd: Path) -> Path:
    return ade_sdd / "doc-aliases.json"


def load_aliases(ade_sdd: Path) -> dict:
    path = _alias_registry_path(ade_sdd)
    if not path.is_file():
        return {"schemaVersion": 1, "aliases": {}}
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {"schemaVersion": 1, "aliases": {}, "corrupt": True}
    value.setdefault("schemaVersion", 1)
    value.setdefault("aliases", {})
    return value


def register_alias(ade_sdd: Path, alias_path: str, canonical_path: str,
                   *, reason: str = "compatibility") -> dict:
    """Register an old path as a pointer; never copies canonical document text."""
    registry = load_aliases(ade_sdd)
    alias = str(Path(alias_path)).replace("\\", "/")
    canonical = str(Path(canonical_path)).replace("\\", "/")
    if alias == canonical:
        raise DocStorageError("E010", "alias path must differ from canonical path")
    registry["aliases"][alias] = {
        "canonical": canonical,
        "reason": reason,
        "updatedAt": datetime.utcnow().strftime("%Y-%m-%dT%H:%M:%SZ"),
    }
    _alias_registry_path(ade_sdd).parent.mkdir(parents=True, exist_ok=True)
    _alias_registry_path(ade_sdd).write_text(
        json.dumps(registry, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    return registry["aliases"][alias]


def resolve_alias(ade_sdd: Path, path: str) -> Path:
    """Resolve at most one alias hop and reject alias cycles."""
    registry = load_aliases(ade_sdd)
    current = str(Path(path)).replace("\\", "/")
    seen = set()
    while current in registry.get("aliases", {}):
        if current in seen:
            raise DocStorageError("E011", f"document alias cycle detected at {current}")
        seen.add(current)
        current = str(registry["aliases"][current].get("canonical") or "")
        if not current:
            raise DocStorageError("E011", "document alias has empty canonical target")
    return Path(current)


def assert_no_duplicate_canonical(ade_sdd: Path, alias_path: str, canonical_path: str) -> None:
    """Reject a compatibility path that contains a second complete正文."""
    alias = Path(alias_path)
    canonical = Path(canonical_path)
    if not alias.is_file() or not canonical.is_file():
        return
    alias_text = alias.read_text(encoding="utf-8", errors="replace")
    canonical_text = canonical.read_text(encoding="utf-8", errors="replace")
    if alias_text.strip() == canonical_text.strip():
        raise DocStorageError("E012", "alias path contains a duplicate canonical document body")
    if "canonical" not in alias_text.lower() and "redirect" not in alias_text.lower():
        raise DocStorageError("E012", "compatibility document must be a canonical pointer, not a second正文")


def resolve_candidates(candidates: list[str | Path], *, require_existing: bool = True) -> Path:
    """Resolve one document candidate; never choose by mtime when ambiguous."""
    paths = [Path(p) for p in candidates if (not require_existing or Path(p).is_file())]
    unique = []
    seen = set()
    for path in paths:
        key = str(path.resolve()).lower()
        if key not in seen:
            unique.append(path)
            seen.add(key)
    if not unique:
        raise DocStorageError("E013", "no canonical document candidate exists")
    if len(unique) > 1:
        raise DocStorageError("E013", "multiple canonical document candidates; explicit migration is required")
    return unique[0]


def migration_dry_run(alias_pairs: list[tuple[str | Path, str | Path]]) -> dict:
    """Return an auditable move/alias/conflict plan without changing files."""
    moves, aliases, conflicts = [], [], []
    for alias, canonical in alias_pairs:
        alias_path, canonical_path = Path(alias), Path(canonical)
        if alias_path.is_file() and canonical_path.is_file():
            try:
                assert_no_duplicate_canonical(Path("."), str(alias_path), str(canonical_path))
            except DocStorageError as exc:
                conflicts.append({"alias": str(alias_path), "canonical": str(canonical_path), "error": str(exc)})
        elif alias_path.is_file() and not canonical_path.exists():
            moves.append({"from": str(alias_path), "to": str(canonical_path)})
        aliases.append({"alias": str(alias_path), "canonical": str(canonical_path)})
    return {"dryRun": True, "moves": moves, "aliases": aliases, "conflicts": conflicts}


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
