"""
paths.py - ae-sdd CLI path helpers.

Resolves master source, project .ae-sdd, state.json, assets, and project docs.
"""
from __future__ import annotations

import json
import os
import re
import sys
import uuid
from pathlib import Path
from typing import Optional


# Keep in sync with source/SKILL.md YAML frontmatter.
MASTER_VERSION = "3.10.1"


# ─── 🆕 v3.10.1 state UUID 前缀（保证目录名/stateMachineId 全局唯一）─────────
# 用户要求：创建 state 时最前面带随机 UUID，防止同业务名撞目录互相覆盖。
# UUID 只在创建时生成一次并持久化，之后所有查找/读取通过已持久化字段定位。
# build_state_machine_name 保持确定性（纯业务名），UUID 前缀在创建入口拼接。

# 标准 UUID v4 格式：8-4-4-4-12 共 36 字符（含 4 个连字符）
_UUID_PREFIX_RE = re.compile(
    r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}-"
)


def generate_state_uuid() -> str:
    """🆕 v3.10.1 生成一个随机 state UUID（标准 v4，36 字符）。

    复用 session.py 的 str(uuid.uuid4()) 惯例，保证与 sessionId 生成方式一致。
    仅在 state 创建入口调用一次，生成后持久化到 stateUuid 字段 + 目录名前缀。
    """
    return str(uuid.uuid4())


def strip_uuid_prefix(name: str) -> str:
    """🆕 v3.10.1 剥离目录名/stateMachineId 的 UUID 前缀，返回纯业务名。

    输入 ``{uuid}-PRD-IM-CS`` -> 返回 ``PRD-IM-CS``；
    输入无 UUID 前缀（如 ``PRD-IM-CS``）-> 原样返回（向后兼容旧 state）。

    用 _UUID_PREFIX_RE 精确匹配 36 字符 UUID + 连字符，不会误剥业务名。
    """
    if not name:
        return name
    m = _UUID_PREFIX_RE.match(name)
    if m:
        return name[m.end():]
    return name


def has_uuid_prefix(name: str) -> bool:
    """🆕 v3.10.1 判断目录名/stateMachineId 是否带 UUID 前缀。"""
    return bool(name and _UUID_PREFIX_RE.match(name))


def compare_versions(installed: Optional[str], master: str = MASTER_VERSION) -> Optional[str]:
    """🆕 v3.4.0：版本对比工具，返回 None 表示一致或无法判断；返回字符串表示落后。

    用于 gate_intercept / health 子命令探测"已安装 SKILL 是否落后于母版"。
    支持 semver 形式（"3.4.0"）；非 semver 字符串统一按 0.0.0 处理。

    Examples:
        compare_versions("3.4.0", "3.4.0")  -> None
        compare_versions("3.2.3", "3.4.0")  -> "installed 3.2.3 < master 3.4.0"
        compare_versions("4.0.0", "3.4.0")  -> None  (新于母版不告警)
        compare_versions(None,   "3.4.0")  -> "installed unknown < master 3.4.0"
    """
    if not installed:
        return f"installed unknown < master {master}"
    if installed == master:
        return None

    def _parse(v: str) -> tuple[int, ...]:
        try:
            return tuple(int(x) for x in v.split(".")[:3])
        except (ValueError, AttributeError):
            return (0, 0, 0)

    if _parse(installed) < _parse(master):
        return f"installed {installed} < master {master}"
    return None  # 新于母版不告警（可能是开发版）


def locate_master_source(start: Optional[Path] = None) -> Optional[Path]:
    """
    Locate the master source directory.

    Priority:
    1. AE_SDD_MASTER environment variable, pointing to source/ or package root.
    2. Current working directory ./source or current directory itself.
    3. Repository/package root relative to this tool.
    4. Installed ~/.claude/skills/ae-sdd and ~/.codex/skills/ae-sdd directories.
    """
    candidates: list[Path] = []

    if env := os.environ.get("AE_SDD_MASTER"):
        env_path = Path(env)
        candidates.append(env_path)
        candidates.append(env_path / "source")

    cwd = Path.cwd()
    candidates.append(cwd / "source")
    candidates.append(cwd)

    cli_path = Path(start) if start else Path(__file__).resolve()
    repo_root = cli_path.parent.parent.parent
    candidates.append(repo_root / "source")
    candidates.append(repo_root)

    home = Path.home()
    candidates.append(home / ".claude" / "skills" / "ae-sdd" / "source")
    candidates.append(home / ".claude" / "skills" / "ae-sdd")
    candidates.append(home / ".codex" / "skills" / "ae-sdd" / "skills" / "ae-sdd" / "source")
    candidates.append(home / ".codex" / "skills" / "ae-sdd" / "skills" / "ae-sdd")
    candidates.append(home / ".codex" / "skills" / "ae-sdd" / "source")
    candidates.append(home / ".codex" / "skills" / "ae-sdd")

    seen: set[Path] = set()
    for cand in candidates:
        resolved = cand.expanduser()
        if resolved in seen:
            continue
        seen.add(resolved)
        if resolved.is_dir() and (resolved / "SKILL.md").is_file():
            return resolved
    return None


def locate_project_ae_sdd(cwd: Optional[Path] = None) -> Optional[Path]:
    """Locate .ae-sdd/ from cwd upward, up to five parent levels."""
    cur = (cwd or Path.cwd()).resolve()
    for _ in range(5):
        cand = cur / ".ae-sdd"
        if cand.is_dir() and (cand / "config.yaml").is_file():
            return cand
        if cur.parent == cur:
            break
        cur = cur.parent
    return None


def _strip_yaml_comment(raw: str) -> str:
    """剥离 YAML 行尾注释，但跳过引号内的 #（C7 修复）。

    旧实现 raw.split("#",1)[0] 会切断引号内含 # 的值（如 description: "see #123"）。
    本函数跟踪单/双引号状态，只剥离引号外的首个 # 之后内容。
    """
    in_single = in_double = False
    for i, ch in enumerate(raw):
        if ch == "'" and not in_double:
            in_single = not in_single
        elif ch == '"' and not in_single:
            in_double = not in_double
        elif ch == "#" and not in_single and not in_double:
            return raw[:i]
    return raw


def read_config(ade_sdd: Path) -> dict:
    """Read .ae-sdd/config.yaml with a tiny key/value parser."""
    cfg_path = ade_sdd / "config.yaml"
    if not cfg_path.is_file():
        return {}
    text = cfg_path.read_text(encoding="utf-8")

    out: dict = {}
    current_section: Optional[str] = None
    for line in text.splitlines():
        line = _strip_yaml_comment(line).rstrip()
        if not line.strip():
            continue
        if line.startswith(" ") and current_section:
            key, _, val = line.strip().partition(":")
            val = val.strip().strip('"').strip("'")
            if val:
                out.setdefault(current_section, {})[key] = val
            continue
        key, _, val = line.partition(":")
        key = key.strip()
        val = val.strip()
        if not val:
            current_section = key
            out.setdefault(key, {})
        else:
            current_section = None
            val = val.strip('"').strip("'")
            out[key] = val
    return out


def state_path(ade_sdd: Path) -> Path:
    return ade_sdd / "state.json"


_WORK_ITEM_DIR_SEP = "--"
_WORK_ITEM_DIR_MAX_LEN = 140


def _normalize_work_item_component(value: str) -> str:
    """Normalize a work-item id/name component for a cross-platform directory."""
    text = (value or "").strip()
    text = re.sub(r'[<>:"/\\|?*\x00-\x1f]+', "-", text)
    text = re.sub(r"\s+", "-", text)
    text = re.sub(r"-{2,}", "-", text).strip(" .-")
    return text or "unnamed"


def work_items_dir(ade_sdd: Path) -> Path:
    """Directory that contains all isolated work-item state machines."""
    return project_root(ade_sdd) / ".auto-engineering"


def work_item_dir_name(top_node: str, features: Optional[dict] = None) -> str:
    """🆕 v3.9.3 废除 v3.8.2 双段：统一走 R6 顶层名（PRD-{特征} / DR-{特征} / Story-{合并编号}）。

    Args:
        top_node: 顶层节点类型 "PRD" / "DR" / "STORY" / "TASK"
        features: {
            "prd_feature": str,    # PRD 顶层特征（如 "IM-CS"）
            "dr_feature": str,     # DR 顶层特征（如 "CS"）
            "story_ids": list,     # Story ID 列表（如 ["STORY-003-BE", "STORY-004-BE"]）
            "task_id": str,        # Task 顶层特征（如 "BUG-LIFE-001"）
        }

    Returns:
        R6 顶层名（如 "PRD-IM-CS" / "DR-CS" / "Story-003-004" / "Task-BUG-LIFE-001"）

    Raises:
        ValueError: top_node 非法或缺关键特征（由 build_state_machine_name 抛）

    Notes:
        v3.9.3 起强制 R6 顶层命名，**不再接受** v3.8.2 双段 `{ID}--{name}` 拼装。
        旧 `work_item_dir_name(id, name)` 签名的 name 形参被废除。
    """
    return build_state_machine_name(top_node, features or {})


def _state_matches_work_item(state_data: dict, token: str) -> bool:
    candidates = {
        state_data.get("workItemId"),
        state_data.get("workItemKey"),
        state_data.get("stateMachineId"),
        state_data.get("stateMachineName"),  # 🆕 v3.10.1 纯业务名（无 UUID 前缀），供按业务名 token 匹配
        state_data.get("currentWorkItem"),
        state_data.get("activeWorkItem"),
        state_data.get("currentStory"),
        state_data.get("activeStory"),
    }
    candidates.update(state_data.get("storyIds") or [])
    candidates.update((state_data.get("storyStates") or {}).keys())
    for dr_id, dr_state in (state_data.get("drStates") or {}).items():
        candidates.add(dr_id)
        if isinstance(dr_state, dict):
            candidates.add(dr_state.get("drId"))
            candidates.update((dr_state.get("storyStates") or {}).keys())
    tokens = {token}
    if "--" in token:
        tokens.add(token.split("--", 1)[0])
    return bool(tokens & {str(c) for c in candidates if c})


def find_work_item_state_path(ade_sdd: Path, work_item_id_or_key: str) -> Optional[Path]:
    """Find an existing isolated state.json by directory key or recorded id.

    🆕 v3.10.1 增加后缀匹配：目录名带 UUID 前缀时（``{uuid}-PRD-IM-CS``），
    传业务名 token（``PRD-IM-CS``）也能命中。
    """
    token = (work_item_id_or_key or "").strip()
    if not token:
        return None

    base = work_items_dir(ade_sdd)
    exact = base / token / "state.json"
    if exact.is_file():
        return exact

    try:
        normalized = work_item_dir_name(token)
    except ValueError:
        normalized = token
    normalized_exact = base / normalized / "state.json"
    if normalized_exact.is_file():
        return normalized_exact

    if not base.is_dir():
        return None

    # 🆕 v3.10.1 后缀匹配模式：token 是纯业务名时，匹配 {uuid}-{token} 目录
    suffix = f"-{normalized}"

    matches: list[Path] = []
    prefix = f"{normalized}{_WORK_ITEM_DIR_SEP}"
    for child in sorted(base.iterdir()):
        if not child.is_dir():
            continue
        state_file = child / "state.json"
        if not state_file.is_file():
            continue
        if (child.name == token or child.name == normalized
                or child.name.startswith(prefix)
                or (has_uuid_prefix(child.name) and child.name.endswith(suffix))):
            matches.append(state_file)
            continue
        try:
            data = json.loads(state_file.read_text(encoding="utf-8"))
        except Exception:
            continue
        if isinstance(data, dict) and _state_matches_work_item(data, token):
            matches.append(state_file)

    unique = list(dict.fromkeys(matches))
    return unique[0] if len(unique) == 1 else None


def work_item_state_path(ade_sdd: Path, top_node: str,
                         features: Optional[dict] = None,
                         state_uuid: Optional[str] = None) -> Path:
    """🆕 v3.9.3 简化：直接走 R6 顶层名，不再探测旧目录。
    🆕 v3.10.1 state_uuid 传入时目录名加 UUID 前缀保证唯一性。

    Args:
        ade_sdd: 项目 .ae-sdd 目录
        top_node: 顶层节点类型 "PRD" / "DR" / "STORY" / "TASK"
        features: 顶层特征字典（见 work_item_dir_name）
        state_uuid: 🆕 v3.10.1 随机 UUID（创建时生成）。传入则目录名变
                    ``{uuid}-{R6业务名}``；不传则纯业务名（查找/向后兼容用）。

    Returns:
        {项目根}/.auto-engineering/{R6 顶层名}/state.json
        state_uuid 非空时顶层名带 UUID 前缀。
    """
    biz_name = work_item_dir_name(top_node, features)
    dir_name = f"{state_uuid}-{biz_name}" if state_uuid else biz_name
    return work_items_dir(ade_sdd) / dir_name / "state.json"


def assets_dir(ade_sdd: Path) -> Path:
    return ade_sdd / "assets"


def overrides_dir(ade_sdd: Path) -> Path:
    return ade_sdd / "overrides"


def reports_dir(ade_sdd: Path) -> Path:
    return ade_sdd / "reports"


def find_asset_file(ade_sdd: Path, project_key: str) -> Optional[Path]:
    """Find the project asset (overview) file under .ae-sdd/assets/.

    🔧 v4.1：总览位置从旧 `{assets}/{key}.assets.md` 升级为新模型
    `{assets}/{key}/{key}.assets.md`（与 document-storage §2.3 工作区级索引一致）。
    查找顺序：新位置优先 → 旧位置回退（向后兼容）。
    """
    assets = assets_dir(ade_sdd)
    if not assets.is_dir():
        return None
    # v4.1 新位置：{assets}/{key}/{key}.assets.md（工作区级索引，含 line 分组子目录）
    new_loc = assets / project_key / f"{project_key}.assets.md"
    if new_loc.is_file():
        return new_loc
    # 旧位置回退：{assets}/{key}.assets.md
    old_loc = assets / f"{project_key}.assets.md"
    return old_loc if old_loc.is_file() else None


def read_asset_field(ade_sdd: Path, project_key: str, field: str) -> Optional[str]:
    """🆕 v4.0：从资产 md §1 读取字段（gitPath / docWorkspacePath / productLine 等）。

    支持 markdown 表格格式（| field | `value` |）和 JSON 块格式（"field": "value"）。
    找不到返回 None（调用方按缺省处理）。
    """
    asset_file = find_asset_file(ade_sdd, project_key)
    if asset_file is None or not asset_file.is_file():
        return None
    try:
        text = asset_file.read_text(encoding="utf-8")
    except OSError:
        return None
    # markdown 表格格式：| field | `value` | 或 | field | value |
    import re
    m = re.search(rf"\|\s*{re.escape(field)}\s*\|\s*`?([^|`]+)`?\s*\|", text)
    if m:
        val = m.group(1).strip().strip("`").strip()
        return val if val else None
    # JSON 块格式："field": "value"
    m = re.search(rf'"{re.escape(field)}"\s*:\s*"([^"]+)"', text)
    if m:
        return m.group(1).strip()
    return None


def resolve_doc_workspace(ade_sdd: Path, project_key: str) -> Optional[Path]:
    """🆕 v4.0：解析文档工作区根路径（document-storage §0.5.1 第四维）。

    优先级：资产 md §1 docWorkspacePath > 缺省回退 gitPath > None。
    用于工程级子文件的就近存放基线：docWorkspacePath/assets/{key}/{module}/。
    """
    doc_ws = read_asset_field(ade_sdd, project_key, "docWorkspacePath")
    if doc_ws:
        return Path(doc_ws)
    git_path = read_asset_field(ade_sdd, project_key, "gitPath")
    if git_path:
        return Path(git_path)
    return None


def resolve_assets_base(ade_sdd: Path, project_key: str) -> Optional[Path]:
    """🆕 v4.1：统一资产根定位入口（document-storage §0.5.3 / §2.3 资产路径 SSOT）。

    返回工程级子文件就近存放的基线目录（对齐 §2.3 新模型）：
      {docWorkspacePath}/.ae-sdd/assets/{projectKey}/

    优先级：资产 md §1 docWorkspacePath > 缺省回退 gitPath > None。
    供 find_module_asset_files / gates 共用，消除各处硬编码。

    注意：.ae-sdd/ 是 ae-sdd 在项目工作区的统一根（与 state.json、secrets 同根），
    资产放其下 assets/ 子目录。docWorkspacePath 缺省回退 gitPath 时，
    即 {gitPath}/.ae-sdd/assets/{key}/（与 ade_sdd 自身所在路径一致）。
    """
    doc_ws = resolve_doc_workspace(ade_sdd, project_key)
    if doc_ws:
        return doc_ws / ".ae-sdd" / "assets" / project_key
    return None


def discover_line_groups(base: Path) -> dict:
    """🆕 v4.1：自动区分 base 目录下的子目录是「module 目录」还是「line 分组目录」。

    判定规则（含同名 .assets.md 即 module；否则若含孙级 module 目录即 line）：
      - module 目录：{base}/{name}/{name}.assets.md 存在 → 归 module 列表
      - line 目录：  {base}/{line}/{name}/{name}.assets.md 存在 → 归 line 字典

    Args:
        base: resolve_assets_base() 返回的 {docWorkspace}/assets/{key}/ 目录

    Returns:
        {
            "flat_modules": [Path, ...],   # 直接 module 子文件（{base}/{m}/{m}.assets.md）
            "line_groups": {line: [Path, ...]},  # line 分组下的 module 文件
        }
        无 module 文件时对应项为空。
    """
    flat_modules: list = []
    line_groups: dict = {}

    if not base.is_dir():
        return {"flat_modules": flat_modules, "line_groups": line_groups}

    for child in sorted(base.iterdir()):
        if not child.is_dir():
            continue
        # 情况 1：本层就是 module 目录（含同名 .assets.md）
        own = child / f"{child.name}.assets.md"
        if own.is_file():
            flat_modules.append(own)
            continue
        # 情况 2：本层是 line 分组目录（孙级含 module 目录）
        line_files = []
        for sub in sorted(child.iterdir()):
            if not sub.is_dir():
                continue
            sub_own = sub / f"{sub.name}.assets.md"
            if sub_own.is_file():
                line_files.append(sub_own)
        if line_files:
            line_groups[child.name] = line_files

    return {"flat_modules": flat_modules, "line_groups": line_groups}


def find_module_asset_files(ade_sdd: Path, project_key: str) -> list:
    """🆕 v4.0 / 🔧 v4.1：发现工程级子文件（总览 + 各工程细节），支持 line 分组。

    返回 [Path, ...]，按"总览在前、子文件在后"排序。子文件发现走三阶段（共存向后兼容）：

      阶段① line 分组：{docWorkspacePath}/assets/{key}/{line}/{module}/{module}.assets.md
              （多业务线项目，如 life 的 2c/admin/common）
      阶段② 单层 module：{docWorkspacePath}/assets/{key}/{module}/{module}.assets.md
              （v4.0 原就近存放规则，单业务线项目）
      阶段③ 旧扁平兼容：.ae-sdd/assets/{key}.*.assets.md
              （历史扁平格式，paths.find_module_asset_files 一直兼容）

    总览：.ae-sdd/assets/{projectKey}.assets.md（find_asset_file），不存在时返回空列表。
    """
    result = []
    overview = find_asset_file(ade_sdd, project_key)
    if overview:
        result.append(overview)

    # 阶段①②：经 docWorkspace 就近存放发现（line 分组 + 单层 module 共用 discover_line_groups）
    base = resolve_assets_base(ade_sdd, project_key)
    if base and base.is_dir():
        discovered = discover_line_groups(base)
        # 阶段① line 分组（按 line 名排序，保证稳定顺序；多业务线项目优先）
        for line_name in sorted(discovered["line_groups"]):
            result.extend(discovered["line_groups"][line_name])
        # 阶段② 单层 module（单业务线项目 / v4.0 原就近存放规则）
        result.extend(discovered["flat_modules"])

    # 阶段③ 兼容旧扁平位置：.ae-sdd/assets/{projectKey}.*.assets.md（排除总览本体）
    assets = assets_dir(ade_sdd)
    if assets.is_dir():
        for f in sorted(assets.iterdir()):
            if (f.name.startswith(f"{project_key}.") and f.name.endswith(".assets.md")
                    and f.name != f"{project_key}.assets.md"):
                result.append(f)

    return result


# ─── 🆕 v4.1 高频路径函数（消除 gates.py / CLI / 其他模块各自硬编码）─────────────
# 这些是扫描报告 B 类发现的"绕过 paths 自拼"高频点，统一收敛到此处。

def config_path(ade_sdd: Path) -> Path:
    """🆕 v4.1：.ae-sdd/config.yaml 路径（消除 gates.py:126 等处 `ade_sdd / "config.yaml"` 自拼）。"""
    return ade_sdd / "config.yaml"


def secrets_dir(ade_sdd: Path) -> Path:
    """🆕 v4.1：.ae-sdd/secrets/ 路径（消除 db_tool.py:58 等处 `ade_sdd / "secrets"` 自拼）。"""
    return ade_sdd / "secrets"


def scripts_dir(master_source: Path) -> Path:
    """🆕 v4.1：母版 scripts/ 目录（消除 gates.py:449 / coding-skill:780 等处自拼）。

    master_source 通常是 source/，scripts/ 在其父目录（仓库根）。
    优先级：master/scripts → master.parent/scripts → master/source/scripts。
    """
    for cand in (master_source / "scripts", master_source.parent / "scripts",
                 master_source / "source" / "scripts"):
        if cand.is_dir():
            return cand
    return master_source.parent / "scripts"  # 缺省指向仓库根 scripts/


def repo_root_from_file(file_path: Path) -> Path:
    """🆕 v4.1：从 __file__ 推导仓库根（消除 CLI 4 处 + plugin_loader 的 `parent.parent.parent` 重复自拼）。

    约定：tools/ 与 scripts/ 均位于仓库根下一层，故 file_path.parent.parent.parent 即仓库根。
    不假设固定层数则需向上找 .git，但此处用约定层数（3 层）保证确定性。
    """
    return file_path.resolve().parent.parent.parent


def project_root(ade_sdd: Path) -> Path:
    """Project root is the parent directory of .ae-sdd/."""
    return ade_sdd.parent


def pending_init_marker(project_dir: Optional[Path] = None) -> Path:
    """返回 ae-sdd 待初始化标记文件路径（用于跨 hook 通信）。

    当用户触发 /ae-sdd 但项目未 init 时，prompt_inject 写入此标记文件，
    gate_intercept 读取它以决定是否拦截未初始化项目的写操作。
    """
    cwd = (project_dir or Path.cwd()).resolve()
    return cwd / ".ae-sdd-pending-init"


def project_design_dir(project_root: Path) -> Path:
    """Project design docs directory for DR, Story, and TestCase docs."""
    return project_root / "design"


def project_task_dir(project_root: Path) -> Path:
    """Project Task docs directory."""
    return project_root / "task"


def doc_search_roots(project_dir: Path) -> list[Path]:
    """🆕 v3.9.10：文档查找的搜索根列表（多根：项目根 + docWorkspace）。

    document-storage-skill §0.5.1 第四维 docWorkspacePath 可与项目根（gitPath）分离，
    文档可能落在任一根部下的 design/（deprecated 旧路径）或 ae-sdd-doc/（新布局）。
    本函数返回去重后的搜索根，供 find_doc / list_docs / gates._find_report_doc 共用，
    消除各处自行拼接 docWorkspace 的重复逻辑（DRY）。

    解析优先级（对齐 resolve_doc_workspace）：assets §1 docWorkspacePath > gitPath 回退。
    无 .ae-sdd/config.yaml 或无 projectKey 时仅返回 [project_dir]（向后兼容）。
    """
    roots: list[Path] = [project_dir]
    ade_sdd = project_dir / ".ae-sdd"
    if ade_sdd.is_dir() and config_path(ade_sdd).is_file():
        cfg = read_config(ade_sdd)
        project_key = cfg.get("projectKey") or cfg.get("project_key")
        if project_key:
            doc_ws = resolve_doc_workspace(ade_sdd, project_key)
            if doc_ws is not None:
                roots.append(doc_ws)
    # 去重（resolve 后比较，容忍 project_dir 与 docWorkspace 指向同一路径）
    seen: set = set()
    unique: list[Path] = []
    for root in roots:
        try:
            key = root.resolve()
        except OSError:
            key = root
        if key in seen:
            continue
        seen.add(key)
        unique.append(root)
    return unique


def find_doc(project_root: Path, story_id: str, suffix: str) -> Optional[Path]:
    """Find the first existing {story_id}{suffix} doc.

    搜索范围（v3.9.10 扩展，覆盖 document-storage 新布局 ae-sdd-doc/）：
      1. 旧 deprecated 路径：{root}/design/{story_id}{suffix}、{root}/{story_id}{suffix}
      2. 新布局：{root}/ae-sdd-doc/ 下任意子目录的 {story_id}{suffix}
         （含 Story/{story_id}.md、Test/{story_id}/{story_id}-testcase.md、
          Coding/{story_id}/{story_id}-CodingPlan.md、iterations/*/{cat}/{story_id}/...）
    root 取自 doc_search_roots（项目根 + docWorkspace），保证 docWorkspace 分离项目也能命中。
    旧路径优先（design/ > 项目根 > ae-sdd-doc/），保持历史项目行为不变。
    """
    pattern = f"{story_id}{suffix}"
    for root in doc_search_roots(project_root):
        # 1. 旧 deprecated 路径（design/ + 项目根）
        for cand in (project_design_dir(root) / pattern, root / pattern):
            if cand.is_file():
                return cand
        # 2. 新布局 ae-sdd-doc/（精确直达目录优先 + rglob 兜底）
        doc_root = root / "ae-sdd-doc"
        if doc_root.is_dir():
            for cand in sorted(doc_root.rglob(pattern)):
                if cand.is_file():
                    return cand
    return None


def list_docs(project_root: Path, story_id: str, suffix: str) -> list[Path]:
    """List {story_id}{suffix} docs under task/ (legacy) and ae-sdd-doc/Task/ (new).

    v3.9.10：补充 document-storage 新布局 ae-sdd-doc/Task/{story_id}/{story_id}{suffix}，
    与旧 task/ 目录共存向后兼容。结果按路径排序、去重。
    """
    pattern = f"{story_id}{suffix}"
    found: list[Path] = []
    seen: set = set()
    for root in doc_search_roots(project_root):
        for cand in sorted(project_task_dir(root).glob(pattern)):
            try:
                key = cand.resolve()
            except OSError:
                key = cand
            if key in seen:
                continue
            seen.add(key)
            found.append(cand)
        # 新布局：ae-sdd-doc/Task/{story_id}/ 下匹配
        task_dir_new = root / "ae-sdd-doc" / "Task" / story_id
        if task_dir_new.is_dir():
            for cand in sorted(task_dir_new.glob(pattern)):
                try:
                    key = cand.resolve()
                except OSError:
                    key = cand
                if key in seen:
                    continue
                seen.add(key)
                found.append(cand)
    return found


# ─── 🆕 v3.9.0 嵌套状态模型：命名 + 向上归入查找 ──────────────────────────────
# R6: 只以最顶层主体特征命名
#   顶层=PRD  → PRD-{PRD特征}        如 PRD-IM-CS
#   顶层=DR   → DR-{DR特征}          如 DR-CS
#   顶层=Story → Story-{合并编号}     如 Story-003-004-005
#
# R2 向上归入：DR/Story 优先归入已存在的上层 state
#   find_nested_state_by_story_id() 扫描现有嵌套 state，定位 Story 所属 state
#   find_nested_state_by_dr_id()    扫描现有嵌套 state，定位 DR 所属 state


def _extract_story_number(story_id: str) -> Optional[str]:
    """从 Story ID 提取编号部分（如 STORY-003-BE → 003）。"""
    if not story_id:
        return None
    m = re.search(r"STORY[-_]?(\d+)", story_id, re.IGNORECASE)
    return m.group(1) if m else None


def build_state_machine_name(top_node: str, features: dict) -> str:
    """R6: 只以最顶层主体特征命名 state。

    Args:
        top_node: 顶层节点类型 "PRD" / "DR" / "STORY"
        features: {
            "prd_feature": str,       # PRD 特征（如 "IM-CS"），top_node=PRD 时必填
            "dr_feature": str,        # DR 特征（如 "CS"），top_node=DR 时必填
            "story_ids": list[str],   # Story ID 列表，top_node=STORY 时必填
        }

    Returns:
        state 标识字符串（如 "PRD-IM-CS" / "DR-CS" / "Story-003-004-005"）

    Raises:
        ValueError: top_node 非法或缺关键特征
    """
    top_node = (top_node or "").upper()
    if top_node == "PRD":
        prd_feature = (features or {}).get("prd_feature", "").strip()
        if not prd_feature:
            raise ValueError("top_node=PRD 必须提供 prd_feature")
        return f"PRD-{prd_feature}"
    if top_node == "DR":
        dr_feature = (features or {}).get("dr_feature", "").strip()
        if not dr_feature:
            raise ValueError("top_node=DR 必须提供 dr_feature")
        return f"DR-{dr_feature}"
    if top_node == "STORY":
        story_ids = (features or {}).get("story_ids") or []
        if not story_ids:
            raise ValueError("top_node=STORY 必须提供 story_ids")
        nums = [n for n in (_extract_story_number(sid) for sid in story_ids) if n]
        if not nums:
            # 无法提取编号，用完整 ID 去重拼接
            nums = list(dict.fromkeys(story_ids))
        return "Story-" + "-".join(nums)
    if top_node == "TASK":
        # 🆕 v3.9.3 TASK 顶层名：Task-{task_id}（如 Task-BUG-LIFE-001）
        task_id = (features or {}).get("task_id") or (features or {}).get("story_ids", [""])[0]
        if not task_id:
            raise ValueError("top_node=TASK 必须提供 task_id")
        return "Task-" + task_id
    if top_node == "BUG":
        # 🆕 v3.10.0 微任务无文档：Bug-{task_id}
        task_id = (features or {}).get("task_id") or ""
        if not task_id:
            raise ValueError("top_node=BUG 必须提供 task_id")
        return "Bug-" + task_id
    if top_node == "PLAN":
        # 🆕 v3.10.0 小任务 CodingPlan 入口：Plan-{plan_id}
        plan_id = (features or {}).get("plan_id") or ""
        if not plan_id:
            raise ValueError("top_node=PLAN 必须提供 plan_id")
        return "Plan-" + plan_id
    raise ValueError(f"未知 top_node: {top_node}（允许: PRD/DR/STORY/TASK/BUG/PLAN）")


def _scan_nested_states(ade_sdd: Path) -> list[tuple[Path, dict]]:
    """扫描 .auto-engineering/ 下所有嵌套 state（stateModel=nested）。

    Returns:
        [(state_path, state_dict), ...] 仅含嵌套 state，flat state 跳过
    """
    base = work_items_dir(ade_sdd)
    if not base.is_dir():
        return []
    results: list[tuple[Path, dict]] = []
    for child in sorted(base.iterdir()):
        if not child.is_dir():
            continue
        state_file = child / "state.json"
        if not state_file.is_file():
            continue
        try:
            data = json.loads(state_file.read_text(encoding="utf-8"))
        except Exception:
            continue
        if isinstance(data, dict) and data.get("stateModel") == "nested":
            results.append((state_file, data))
    return results


def find_nested_state_by_story_id(ade_sdd: Path,
                                   story_id: str) -> Optional[tuple[Path, dict]]:
    """R2/R5: 按 Story ID 查找其所属的嵌套 state。

    扫描所有嵌套 state 的 storyStates 键，命中返回 (state_path, state_dict)。

    Args:
        ade_sdd: 项目 .ae-sdd 目录
        story_id: 要查找的 Story ID（如 "STORY-003-BE"）

    Returns:
        (state_path, state_dict) 或 None（未找到/无嵌套 state）
    """
    if not story_id:
        return None
    for state_path, data in _scan_nested_states(ade_sdd):
        story_states = data.get("storyStates") or {}
        if story_id in story_states:
            return (state_path, data)
        for dr_state in (data.get("drStates") or {}).values():
            if isinstance(dr_state, dict) and story_id in (dr_state.get("storyStates") or {}):
                return (state_path, data)
    return None


def find_nested_state_by_dr_id(ade_sdd: Path,
                                dr_id: str) -> Optional[tuple[Path, dict]]:
    """R2: 按 DR ID 查找其所属的嵌套 state（用于 DR 向上归入 PRD state）。

    扫描所有嵌套 state 的 drState.drId，命中返回 (state_path, state_dict)。

    Args:
        ade_sdd: 项目 .ae-sdd 目录
        dr_id: 要查找的 DR ID

    Returns:
        (state_path, state_dict) 或 None
    """
    if not dr_id:
        return None
    for state_path, data in _scan_nested_states(ade_sdd):
        dr_states = data.get("drStates") or {}
        if dr_id in dr_states:
            return (state_path, data)
        for dr_state in dr_states.values():
            if isinstance(dr_state, dict) and dr_state.get("drId") == dr_id:
                return (state_path, data)
        dr_state = data.get("drState") or {}
        if dr_state.get("drId") == dr_id:
            return (state_path, data)
    return None


def find_nested_state_by_prd_id(ade_sdd: Path,
                                 prd_id: str) -> Optional[tuple[Path, dict]]:
    """R2: 按 PRD ID 查找嵌套 state（用于 DR/Story 向上归入 PRD state）。

    扫描所有嵌套 state 的 prdState.prdId，命中返回 (state_path, state_dict)。
    """
    if not prd_id:
        return None
    for state_path, data in _scan_nested_states(ade_sdd):
        prd_state = data.get("prdState") or {}
        if prd_state.get("prdId") == prd_id:
            return (state_path, data)
    return None


# ─── 🆕 v3.9.3 父级文档字段抽取 + 关联性验证（用于 R2 强制向上归入）───────────
# 抽取自 Story/DR 文档元信息章节的"来源 DR" / "来源 PRD" 字段，验证：
#   1. 父级文档是否真实存在于 design/ 目录
#   2. 父级文档是否真的"包含"当前节点（关联性验证）
# 找不到 / 关联性不对 → verify_parent_claim 返回 (False, reason)
#   视为"无父级"，递归 R2 算法继续按无父级路径处理


_PARENT_CLAIM_PATTERNS = {
    # Story 模板元信息章节（list-style）："- 来源 PRD: PRD-001" / "- 来源 DR: DR-005"
    "story": [
        (re.compile(r"^\s*-\s*(?:Source\s+)?PRD\s*[:：]\s*([A-Za-z0-9\-_]+)", re.IGNORECASE | re.MULTILINE), "prd"),
        (re.compile(r"^\s*-\s*(?:Source\s+)?DR\s*[:：]\s*([A-Za-z0-9\-_]+)", re.IGNORECASE | re.MULTILINE), "dr"),
        (re.compile(r"来源\s*PRD\s*[:：]\s*([A-Za-z0-9\-_]+)", re.IGNORECASE), "prd"),
        (re.compile(r"来源\s*DR\s*[:：]\s*([A-Za-z0-9\-_]+)", re.IGNORECASE), "dr"),
    ],
    # DR 模板元信息章节：只抽"PRD: PRD-001"作为父级；"DR ID: DR-005"是自身标识，不抽
    "dr": [
        (re.compile(r"^-\s*PRD\s*[:：]\s*([A-Za-z0-9\-_]+)", re.IGNORECASE | re.MULTILINE), "prd"),
    ],
}


def extract_parent_claim(doc_path: Path,
                         doc_kind: str = "story") -> tuple[Optional[str], Optional[str]]:
    """🆕 v3.9.3 从 Story/DR 文档元信息章节抽取父级声明。

    Args:
        doc_path: 文档绝对路径
        doc_kind: "story" 或 "dr" — 选择字段提取模式

    Returns:
        (parent_prd, parent_dr) — 任一可空；标准化大写
    """
    if doc_path is None or not doc_path.is_file():
        return (None, None)
    try:
        text = doc_path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return (None, None)

    patterns = _PARENT_CLAIM_PATTERNS.get(doc_kind) or _PARENT_CLAIM_PATTERNS["story"]
    parent_prd: Optional[str] = None
    parent_dr: Optional[str] = None
    for pattern, key in patterns:
        m = pattern.search(text)
        if not m:
            continue
        value = m.group(1).strip().upper()
        if key == "prd" and not value.startswith("PRD-"):
            value = f"PRD-{value}"
        if key == "dr" and not value.startswith("DR-"):
            value = f"DR-{value}"
        if key == "prd" and not parent_prd:
            parent_prd = value
        elif key == "dr" and not parent_dr:
            parent_dr = value
    return (parent_prd, parent_dr)


def _find_design_doc(design_dir: Path, doc_id: str) -> Optional[Path]:
    """🆕 v3.9.3 在 design/ 目录按 ID 找文档。

    匹配模式：
      - PRD-001 → design/PRD-001-*.md / design/PRD-001.md
      - DR-005  → design/DR-005-*.md / design/DR-005.md
      - 大小写不敏感
    """
    if not design_dir or not design_dir.is_dir() or not doc_id:
        return None
    upper_id = doc_id.upper()
    # 先精确前缀匹配
    for child in sorted(design_dir.iterdir()):
        if not child.is_file() or not child.suffix == ".md":
            continue
        name_upper = child.name.upper()
        if name_upper == f"{upper_id}.MD" or name_upper.startswith(f"{upper_id}-"):
            return child
    return None


def verify_parent_claim(parent_type: str, parent_id: str,
                        design_dir: Path,
                        child_id: str = "") -> tuple[bool, str]:
    """🆕 v3.9.3 验证父级文档存在 + 关联性对。

    Args:
        parent_type: "PRD" 或 "DR"
        parent_id: 父级 ID（如 "DR-005"）
        design_dir: design/ 目录
        child_id: 当前节点 ID（如 "STORY-006-BE"），用于关联性验证

    Returns:
        (ok, reason)：
          ok=True, reason="ok" → 真有父级
          ok=False, reason="doc_not_found" → 父级文档不存在
          ok=False, reason="relation_mismatch" → 文档存在但关联性不对
          ok=False, reason="invalid_args" → 参数非法
    """
    if not parent_type or not parent_id:
        return (False, "invalid_args")
    parent_type = parent_type.upper()
    parent_id = parent_id.strip().upper()
    if parent_type not in ("PRD", "DR"):
        return (False, "invalid_args")

    doc_path = _find_design_doc(design_dir, parent_id)
    if doc_path is None:
        return (False, "doc_not_found")

    # 关联性验证：父级文档需在子节点列表中提及 child_id
    if not child_id:
        return (True, "ok")  # 没传 child_id 跳过关联性验证

    try:
        text = doc_path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return (True, "ok")  # 读失败不阻断（视为 OK 但给 warn）

    # 关联性关键词：DR 文档中需出现 child_id 完整 ID
    # PRD 文档中需出现 parent_id（即 DR ID 出现在 PRD 文档中），但 verify 关注的是 DR/Story 的父级
    child_upper = child_id.strip().upper()
    if child_upper and child_upper in text.upper():
        return (True, "ok")
    # 弱关联：去掉 -BE/-FE 后缀再查
    short = re.sub(r"[-_][A-Z]+$", "", child_upper)
    if short and short != child_upper and short in text.upper():
        return (True, "ok")
    return (False, "relation_mismatch")
