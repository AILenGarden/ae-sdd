"""
document_storage.py - 文档存放 API 代码层实现（document-storage-skill §0.6 的 Python SSOT）。

🆕 v4.1（2026-06-27，路径治理修订）：本模块把 document-storage-skill.md §0.6 声明的
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
# 模板占位符：{docWorkspace} {storyId} {docId} {major} {minor}
# 未带版本号的文档（设计类）原地更新；带版本号的（事件报告）每次写新版本。
_PATH_TEMPLATES: dict[str, tuple[str, bool, str]] = {
    "PRD":               ("{docWorkspace}/ae-sdd-doc/PRD/{docId}.md",                  False, "PRD"),
    "RA":                ("{docWorkspace}/ae-sdd-doc/RA/{docId}.md",                   False, "RA"),
    "DR":                ("{docWorkspace}/ae-sdd-doc/DR/{docId}.md",                   False, "DR"),
    "STORY":             ("{docWorkspace}/ae-sdd-doc/Story/{docId}.md",                False, "Story"),
    "TASK":              ("{docWorkspace}/ae-sdd-doc/Task/{storyId}/{docId}.md",       False, "Task"),
    "CODING_PLAN":       ("{docWorkspace}/ae-sdd-doc/Coding/{storyId}/{storyId}-CodingPlan.md", False, "Coding"),
    "CODING_REPORT":     ("{docWorkspace}/ae-sdd-doc/Coding/{storyId}/{storyId}-CodingReport-v{major}-r{minor}.md", True, "Coding"),
    "TESTCASE":          ("{docWorkspace}/ae-sdd-doc/Test/{storyId}/{storyId}-testcase.md",  False, "Test"),
    "TEST_REPORT":       ("{docWorkspace}/ae-sdd-doc/Test/{storyId}/{storyId}-Report-v{major}-r{minor}.md", True, "Test"),
    "CODE_REVIEW":       ("{docWorkspace}/ae-sdd-doc/CR/{storyId}/{storyId}-CodeReview-v{major}-r{minor}.md", True, "CR"),
    "TRACEABILITY":      ("{docWorkspace}/ae-sdd-doc/Coding/{storyId}/{storyId}-追溯矩阵-v{major}-r{minor}.md", True, "Coding"),
    "STORY_REVIEW":      ("{docWorkspace}/ae-sdd-doc/CR/{storyId}/{storyId}-StoryReviewReport-r{minor}.md", True, "CR"),
    "REVIEW_UPDATEPLAN": ("{docWorkspace}/ae-sdd-doc/CR/{storyId}/{storyId}-StoryReviewUpdatePlan-r{minor}.md", True, "CR"),
    "TASK_SMALL":        ("{docWorkspace}/ae-sdd-doc/iterations/{iterDate}/Task/{docId}/",  False, "Task"),
    "PLAN_MICRO":        ("{docWorkspace}/ae-sdd-doc/iterations/{iterDate}/Coding/{docId}/", False, "Coding"),
    "PROPOSAL":          ("{docWorkspace}/ae-sdd-doc/CR/{storyId}/{storyId}-Proposal.md",  False, "CR"),
}


# ─── 错误码（对齐 §0.6.15）────────────────────────────────────────────────────
class DocStorageError(Exception):
    """document_storage 统一异常，带错误码。"""
    def __init__(self, code: str, message: str):
        self.code = code
        super().__init__(f"[{code}] {message}")


# ─── 返回数据类 ────────────────────────────────────────────────────────────────
@dataclass
class ResolvedPath:
    """resolve_path() 返回值（对齐 §0.6.1 ResolvedPath 接口）。"""
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
    """save_doc() 返回值（对齐 §0.6.7 SaveResult）。"""
    success: bool
    new_version: Optional[str]
    changelog_entry: Optional[str]
    full_path: str
    error: Optional[str] = None


@dataclass
class IterationChoice:
    """choose_iteration() 返回值（对齐 §0.6.8 IterationChoice）。"""
    date: str
    strength: str  # 'strong' | 'weak' | 'none'
    reasoning: str


# ═══════════════════════════════════════════════════════════════════════════════
# P1 核心 API（纯路径拼接，确定性）
# ═══════════════════════════════════════════════════════════════════════════════

def get_git_path(ade_sdd: Path, project_key: str) -> Optional[str]:
    """§0.6.2：返回项目根绝对路径（从 assets.md §1 gitPath 读取）。

    E001（无资产）/ E002（字段空）→ 返回 None（调用方按缺失处理）。
    """
    val = paths.read_asset_field(ade_sdd, project_key, "gitPath")
    return val if val else None


def get_service_root(ade_sdd: Path, project_key: str, service_name: str) -> Optional[str]:
    """§0.6.3：返回微服务根 = {gitPath}/{service_name}。

    E004（微服务名不在 §2 列表）由调用方按需校验；本函数仅拼接。
    """
    git_path = get_git_path(ade_sdd, project_key)
    if git_path is None:
        return None
    return str(Path(git_path) / service_name)


def get_constraints(ade_sdd: Path, project_key: str) -> dict:
    """§0.6.4：返回约束文档名 → 完整路径映射。

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
    """§0.6.5：返回项目资产文件路径列表（总览 + 工程级子文件）。

    取代 SKILL 内直接写死的 assets/{projectKey}/ 路径。
    复用 paths.find_module_asset_files（v4.1 支持 line 分组发现）。
    """
    return paths.find_module_asset_files(ade_sdd, project_key)


def resolve_path(ade_sdd: Path, project_key: str, intent: str,
                 story_id: Optional[str] = None, doc_id: Optional[str] = None,
                 service_name: Optional[str] = None, task_name: Optional[str] = None,
                 version: Optional[dict] = None,
                 iteration_date: Optional[str] = None) -> ResolvedPath:
    """§0.6.1：核心 API，推导文档完整落地路径。

    步骤（对齐 §0.6.1 行为）：
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
    version_suffix = ""
    if has_version:
        v = version or {"major": 1, "minor": 1}
        version_suffix = f"{v['major']}.{v['minor']}"

    # 4. 替换占位符
    full = template.format(
        docWorkspace=str(doc_ws).replace("\\", "/"),
        storyId=story_id or "",
        docId=doc_id or task_name or "",
        major=version.split(".")[0] if version and has_version else (version or {}).get("major", 1),
        minor=(version or {}).get("minor", 1) if has_version else 1,
        iterDate=iter_date or datetime.now().strftime("%Y-%m-%d"),
    )
    p = Path(full.replace("/", "\\" if "\\" in str(doc_ws) else "/") if False else full)
    # 统一用 Path，跨平台
    full_path_obj = Path(full)

    scope = "service" if service_name else "project"
    changelog_path = str(full_path_obj.parent / f"{full_path_obj.stem}-changelog.md")

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

_VERSION_RE = re.compile(r"-v(\d+)\.(\d+)(?:-r(\d+))?\.md$")


def get_latest_version(doc_dir: Path, stem_prefix: str) -> Optional[tuple[int, int]]:
    """§0.6.11：返回 {doc_dir} 下 {stem_prefix}-v{N}.{m}.md 的最新版本号。

    Returns:
        (major, minor) 最大者；无版本文件返回 None。
    """
    if not doc_dir.is_dir():
        return None
    best: Optional[tuple[int, int]] = None
    for f in doc_dir.glob(f"{stem_prefix}-v*.*.md"):
        m = _VERSION_RE.search(f.name)
        if m:
            cur = (int(m.group(1)), int(m.group(2)))
            if best is None or cur > best:
                best = cur
    return best


def get_changelog(changelog_path: Path) -> list[str]:
    """§0.6.12：返回 ChangeLog 文件内容（行列表），不存在返回空。"""
    if not changelog_path.is_file():
        return []
    return changelog_path.read_text(encoding="utf-8", errors="replace").splitlines()


def save_doc(ade_sdd: Path, project_key: str, intent: str, content: str,
             story_id: Optional[str] = None, doc_id: Optional[str] = None,
             version: Optional[dict] = None,
             changelog_note: Optional[str] = None) -> SaveResult:
    """§0.6.7：统一文档保存入口（版本号自增 + ChangeLog + 目录创建 + .gitignore）。

    步骤（对齐 §0.6.7）：
      1. resolve_path 推导路径
      2. 若带版本号且未显式传 version → get_latest_version 自增（E007：重入必须递增）
      3. 写文件（旧版本保留）
      4. 追加 ChangeLog
      5. 返回 SaveResult
    """
    try:
        resolved = resolve_path(ade_sdd, project_key, intent,
                                story_id=story_id, doc_id=doc_id, version=version)
    except DocStorageError as e:
        return SaveResult(False, None, None, "", error=str(e))

    full_path = Path(resolved.full_path)
    full_path.parent.mkdir(parents=True, exist_ok=True)

    new_version = None
    if resolved.version_suffix:
        # 带版本号：未显式指定时自增
        if version is None:
            stem = full_path.stem.rsplit("-v", 1)[0] if "-v" in full_path.stem else full_path.stem
            latest = get_latest_version(full_path.parent, stem)
            if latest:
                new_version = f"{latest[0]}.{latest[1] + 1}"
            else:
                new_version = "1.1"
        else:
            new_version = resolved.version_suffix

    full_path.write_text(content, encoding="utf-8")

    changelog_entry = None
    if changelog_note:
        cl_path = Path(resolved.changelog_path)
        entry = f"- {datetime.now().strftime('%Y-%m-%d %H:%M')} v{new_version or 'N/A'}: {changelog_note}"
        with cl_path.open("a", encoding="utf-8") as f:
            f.write(entry + "\n")
        changelog_entry = entry

    update_storing_index(ade_sdd, project_key, resolved.scope,
                         {"category": resolved.storing_index_update["category"],
                          "docType": intent, "fullPath": str(full_path)})

    return SaveResult(True, new_version, changelog_entry, str(full_path))


def update_storing_index(ade_sdd: Path, project_key: str, scope: str, entry: dict) -> None:
    """§0.6.6：更新 STORING.md 索引（项目级 ae-sdd-doc/STORING.md）。

    小任务兼容旧路径 {gitPath}/{serviceName}/.ae-task/Task-xxx/STORING.md。
    追加一行（幂等：同 fullPath 不重复追加）。
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
    """§0.6.13：检查 .gitignore 是否含 pattern，无则追加。返回是否新增。

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
    """§0.6.9：业务关联性判定（启发式：关键词匹配）。

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
    """§0.6.10：逻辑关联性判定（启发式：与 business 同算法，语义上关注技术标签）。

    本实现与 check_business_coherence 同算法（均基于关键词命中），
    调用方可对两类标签做不同标注后分别传入。
    """
    return check_business_coherence(doc_tags, iteration_dir)


def choose_iteration(ade_sdd: Path, project_key: str, doc_signature: str,
                     doc_tags: Optional[list[str]] = None,
                     today: Optional[str] = None) -> IterationChoice:
    """§0.6.8：迭代归属判定（启发式：关联性 + 时间衰减）。

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
    """§0.6.14：旧路径迁移（design/ .ae-task/ .ae-plan/ .spec/ → ae-sdd-doc/）。

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
