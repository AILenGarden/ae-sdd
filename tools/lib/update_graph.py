"""
update_graph.py — ae-sdd 更新依赖图谱检查器（v3.2）

把 ae-sdd-update-skill 的"更新依赖图谱"从纸面规则变成可执行检查。
对标 gates.py 的 GateResult 模式：每项检查产出一个 UpdateCheckResult。

5 类检查：
  UC-01 版本号一致性    SKILL.md / paths.py / README.md 三处版本号必须一致
  UC-02 门禁注册一致性  GATE_REGISTRY 每个 id 都在 CHECK_FUNCS 或 check_all 特判中
  UC-03 命令契约闭环    SKILL.md 引用的 `ae-sdd <cmd>` 都在 CLI add_parser 注册
  UC-04 扫描器分发一致性 scripts/*_scan.py 都在 build_dist.py runtime_scripts 白名单
  UC-05 健康度清单覆盖  ae-sdd-update-skill 健康度清单含本仓库关键组件

无第三方依赖，可独立运行。
"""
from __future__ import annotations

import fnmatch
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional


@dataclass
class UpdateCheckResult:
    """单项更新检查结果"""
    check_id: str
    name: str
    severity: str            # "error" | "warn"
    pass_: bool
    message: str
    fix: Optional[str] = None
    details: dict = field(default_factory=dict)


# ─── UC-01 版本号一致性 ──────────────────────────────────────────────────────
def _extract_skill_version(skill_md: Path) -> Optional[str]:
    """从 source/SKILL.md frontmatter 提取 version 字段。"""
    if not skill_md.is_file():
        return None
    text = skill_md.read_text(encoding="utf-8", errors="replace")
    m = re.search(r"^version:\s*(\S+)", text, re.MULTILINE)
    return m.group(1) if m else None


def _extract_paths_master_version(paths_py: Path) -> Optional[str]:
    """从 tools/lib/paths.py 提取 MASTER_VERSION。"""
    if not paths_py.is_file():
        return None
    text = paths_py.read_text(encoding="utf-8", errors="replace")
    m = re.search(r'MASTER_VERSION\s*=\s*"([^"]+)"', text)
    return m.group(1) if m else None


def _extract_readme_version(readme: Path) -> Optional[str]:
    """从 README.md:5 版本行提取版本号（v3.2.1 → 3.2.1）。"""
    if not readme.is_file():
        return None
    text = readme.read_text(encoding="utf-8", errors="replace")
    # 版本号在 line 5 附近，格式 "> **版本：** v3.2.1（...）"
    m = re.search(r"版本[：:]\s*\*?\*?\s*v?(\d+\.\d+\.\d+)", text)
    return m.group(1) if m else None


def check_uc01_version(repo_root: Path) -> UpdateCheckResult:
    """UC-01 版本号一致性：SKILL.md / paths.py / README.md 三处版本号必须一致。"""
    name = "版本号一致性"
    skill_v = _extract_skill_version(repo_root / "source" / "SKILL.md")
    paths_v = _extract_paths_master_version(repo_root / "tools" / "lib" / "paths.py")
    readme_v = _extract_readme_version(repo_root / "README.md")

    versions = {"source/SKILL.md": skill_v, "tools/lib/paths.py": paths_v,
                "README.md": readme_v}
    missing = {k: v for k, v in versions.items() if v is None}
    if missing:
        return UpdateCheckResult("UC-01", name, "error", False,
                                 f"无法提取版本号：{list(missing.keys())}",
                                 "检查三处版本号格式是否被改动",
                                 details={"versions": versions, "missing": list(missing.keys())})

    unique = {skill_v, paths_v, readme_v}
    if len(unique) > 1:
        return UpdateCheckResult("UC-01", name, "error", False,
                                 f"版本号漂移：SKILL.md={skill_v} / paths.py={paths_v} / README.md={readme_v}",
                                 "统一三处版本号（取最高值），改 SKILL.md frontmatter 后同步 paths.py MASTER_VERSION + README.md:5",
                                 details={"versions": versions, "unique": sorted(unique)})

    return UpdateCheckResult("UC-01", name, "error", True,
                             f"版本号一致：{skill_v}",
                             details={"version": skill_v})


# ─── 版本号 bump（UC-01 的操作侧，Agent 可调）────────────────────────────────
def _write_skill_version(skill_md: Path, new_version: str) -> None:
    """写 SKILL.md frontmatter 的 version 字段。"""
    text = skill_md.read_text(encoding="utf-8", errors="replace")
    new_text, n = re.subn(
        r"(^version:\s*)\S+",
        rf"\g<1>{new_version}",
        text,
        count=1,
        flags=re.MULTILINE,
    )
    if n == 0:
        raise ValueError(f"SKILL.md frontmatter 未找到 version 字段，无法 bump")
    skill_md.write_text(new_text, encoding="utf-8")


def _write_paths_master_version(paths_py: Path, new_version: str) -> None:
    """写 paths.py 的 MASTER_VERSION。"""
    text = paths_py.read_text(encoding="utf-8", errors="replace")
    new_text, n = re.subn(
        r'(MASTER_VERSION\s*=\s*")([^"]+)(")',
        rf'\g<1>{new_version}\g<3>',
        text,
        count=1,
    )
    if n == 0:
        raise ValueError(f"paths.py 未找到 MASTER_VERSION，无法 bump")
    paths_py.write_text(new_text, encoding="utf-8")


def _write_readme_version(readme: Path, new_version: str) -> None:
    """写 README.md:5 的版本号（v3.2.x 格式，保留括号说明原文）。

    匹配 `版本：** v3.2.x` 或 `版本：v3.2.x`，只替换版本号部分。
    """
    text = readme.read_text(encoding="utf-8", errors="replace")
    new_text, n = re.subn(
        r"(版本[：:]\s*\*?\*?\s*v)(\d+\.\d+\.\d+)",
        rf"\g<1>{new_version}",
        text,
        count=1,
    )
    if n == 0:
        raise ValueError(f"README.md 未找到版本行（格式：版本：vX.Y.Z），无法 bump")
    readme.write_text(new_text, encoding="utf-8")


def bump_version(repo_root: Path, new_version: str) -> dict:
    """同步三处版本号：SKILL.md frontmatter / paths.py MASTER_VERSION / README.md:5。

    写入后立即调 UC-01 校验一致性，不一致则报错（文件已改，需人工回滚）。
    版本号格式校验：必须为 X.Y.Z（数字.数字.数字）。

    Args:
        repo_root: 仓库根目录
        new_version: 新版本号，如 "3.2.5"（不带 v 前缀）
    Returns:
        {old, new, written: [文件列表], verified: bool}
    Raises:
        ValueError: 版本号格式非法或写入后 UC-01 校验失败
    """
    # 格式校验
    if not re.fullmatch(r"\d+\.\d+\.\d+", new_version):
        raise ValueError(f"版本号格式非法（需 X.Y.Z）：{new_version}")

    skill_md = repo_root / "source" / "SKILL.md"
    paths_py = repo_root / "tools" / "lib" / "paths.py"
    readme = repo_root / "README.md"

    old = _extract_skill_version(skill_md)
    if old == new_version:
        return {"old": old, "new": new_version, "written": [], "verified": True,
                "skipped": "版本号未变化"}

    written = []
    _write_skill_version(skill_md, new_version)
    written.append("source/SKILL.md")
    _write_paths_master_version(paths_py, new_version)
    written.append("tools/lib/paths.py")
    _write_readme_version(readme, new_version)
    written.append("README.md")

    # 写入后立即校验
    result = check_uc01_version(repo_root)
    verified = result.pass_
    if not verified:
        raise ValueError(
            f"bump 后 UC-01 校验失败：{result.message}。"
            f"文件已改（{written}），需人工核对或回滚。"
        )

    return {"old": old, "new": new_version, "written": written, "verified": verified}


# ─── UC-02 门禁注册一致性 ────────────────────────────────────────────────────
def check_uc02_gates_registry(repo_root: Path) -> UpdateCheckResult:
    """UC-02 门禁注册一致性：GATE_REGISTRY 每个 id 都在 CHECK_FUNCS 或 check_all 特判中。"""
    name = "门禁注册一致性"
    sys.path.insert(0, str(repo_root / "tools"))
    try:
        from lib import gates  # noqa: E402
    except Exception as e:
        return UpdateCheckResult("UC-02", name, "error", False,
                                 f"无法 import gates：{e}",
                                 "检查 tools/lib/gates.py 是否可 import")
    finally:
        # 清理 sys.path 避免污染
        if str(repo_root / "tools") in sys.path:
            sys.path.remove(str(repo_root / "tools"))

    registry_ids = {g["id"] for g in gates.GATE_REGISTRY}
    checkfunc_ids = set(gates.CHECK_FUNCS.keys())
    # check_all 里特判的 id（G-00 / G-09 / G-RA-4 等，不在 CHECK_FUNCS 但在 check_all 手动分发）
    gates_py_text = (repo_root / "tools" / "lib" / "gates.py").read_text(encoding="utf-8", errors="replace")
    special_handled = set()
    for m in re.finditer(r'g\["id"\]\s*==\s*"([^"]+)"', gates_py_text):
        special_handled.add(m.group(1))

    # G-00 在 check_all 里是 check_g00 调用，但不在 CHECK_FUNCS（它签名不同）
    covered = checkfunc_ids | special_handled | {"G-00"}
    uncovered = registry_ids - covered

    if uncovered:
        return UpdateCheckResult("UC-02", name, "error", False,
                                 f"GATE_REGISTRY 有门禁未注册到 CHECK_FUNCS 或 check_all 特判：{sorted(uncovered)}",
                                 f"在 CHECK_FUNCS 注册 {sorted(uncovered)}，或在 check_all 加特判分支",
                                 details={"registry": sorted(registry_ids), "uncovered": sorted(uncovered)})

    return UpdateCheckResult("UC-02", name, "error", True,
                             f"门禁注册一致：{len(registry_ids)} 个门禁全部覆盖",
                             details={"n_gates": len(registry_ids), "covered": sorted(covered & registry_ids)})


# ─── UC-03 命令契约闭环 ──────────────────────────────────────────────────────
# SKILL.md 引用但 CLI 未实现的历史"未来命令"（v3.2 前就声明，本次不实现，只 warn）
# 注：`assets` 组已于 2026-06-24 实现 query/outline/section/stats（ES 化索引），
# 从此集合移除；assets check/generate/update/audit/read 仍走 SKILL 协议，后续迭代补。
# 注：`init` 已于 2026-06-25（v3.2.5）挂到 CLI（subprocess 调 scripts/init.py），
# 从此集合移除，UC-03 warn 清零该项。
HISTORICAL_UNIMPLEMENTED = {"fork", "run", "skill", "sync-tools"}


def _extract_cli_commands(cli_path: Path) -> set:
    """从 tools/bin/ae-sdd 提取所有 add_parser 注册的子命令（含二级）。"""
    if not cli_path.is_file():
        return set()
    text = cli_path.read_text(encoding="utf-8", errors="replace")
    # 顶级 sub.add_parser("xxx") + 二级 xxx_sub.add_parser("yyy")
    cmds = set(re.findall(r'\.add_parser\("([a-z][a-z-]*)"', text))
    return cmds


def _extract_skill_referenced_commands(skill_md: Path) -> set:
    """从 source/SKILL.md 提取所有 `ae-sdd <cmd>` 引用的命令（排除 YAML frontmatter）。"""
    if not skill_md.is_file():
        return set()
    text = skill_md.read_text(encoding="utf-8", errors="replace")
    # 跳过 YAML frontmatter（--- ... ---），只扫正文
    body = text
    if text.startswith("---"):
        end = text.find("\n---", 3)
        if end != -1:
            body = text[end + 4:]
    # 匹配 ae-sdd <cmd>，取一级 cmd（cmd 必须是合法命令词，排除 description/version 等字段名）
    cmds = set(re.findall(r"ae-sdd\s+([a-z][a-z-]*)", body))
    # 排除 YAML 字段名误匹配（description 等不会出现在正文 ae-sdd 后，但防御性排除）
    cmds -= {"description", "version", "name", "main_entry"}
    return cmds


def check_uc03_command_contract(repo_root: Path) -> UpdateCheckResult:
    """UC-03 命令契约闭环：SKILL.md 引用的命令都在 CLI 实现。"""
    name = "命令契约闭环"
    cli_cmds = _extract_cli_commands(repo_root / "tools" / "bin" / "ae-sdd")
    referenced = _extract_skill_referenced_commands(repo_root / "source" / "SKILL.md")

    # CLI 实现的命令（含 help/version 等别名）
    # 注意：CLI 里 version 子命令存在，SKILL 可能引用 "v" 别名
    missing = referenced - cli_cmds - {"v"}  # v 是 version 别名

    # 区分历史遗留（warn）vs 本次新增未实现（error）
    historical = missing & HISTORICAL_UNIMPLEMENTED
    new_missing = missing - HISTORICAL_UNIMPLEMENTED

    if new_missing:
        return UpdateCheckResult("UC-03", name, "error", False,
                                 f"SKILL.md 引用但 CLI 未实现的命令（本次新增）：{sorted(new_missing)}",
                                 f"在 tools/bin/ae-sdd 用 add_parser 注册 {sorted(new_missing)}",
                                 details={"missing": sorted(missing), "historical": sorted(historical),
                                          "new_missing": sorted(new_missing)})

    if historical:
        return UpdateCheckResult("UC-03", name, "warn", True,
                                 f"历史遗留未实现命令（warn，本次不实现）：{sorted(historical)}",
                                 f"后续迭代实现 {sorted(historical)}，或在 SKILL.md 标注'未来命令'",
                                 details={"historical": sorted(historical)})

    return UpdateCheckResult("UC-03", name, "error", True,
                             f"命令契约闭环：SKILL.md 引用的命令全部实现",
                             details={"referenced": sorted(referenced), "implemented": sorted(cli_cmds & referenced)})


# ─── UC-04 扫描器分发一致性 ──────────────────────────────────────────────────
def check_uc04_scanner_distribution(repo_root: Path) -> UpdateCheckResult:
    """UC-04 扫描器分发一致性：scripts/*_scan.py 都在 build_dist.py runtime_scripts 白名单。"""
    name = "扫描器分发一致性"
    scripts_dir = repo_root / "scripts"
    build_dist = repo_root / "scripts" / "build_dist.py"

    if not scripts_dir.is_dir() or not build_dist.is_file():
        return UpdateCheckResult("UC-04", name, "error", False,
                                 "scripts/ 或 build_dist.py 不存在")

    # 找所有 *_scan.py
    scanners = sorted(p.name for p in scripts_dir.glob("*_scan.py") if p.is_file())

    # 提取 build_dist.py 白名单
    bd_text = build_dist.read_text(encoding="utf-8", errors="replace")
    # runtime_scripts = [...] 块内的文件名
    whitelist_match = re.search(r'runtime_scripts\s*=\s*\[(.*?)\]', bd_text, re.DOTALL)
    whitelist = set()
    if whitelist_match:
        whitelist = set(re.findall(r'"([^"]+\.py)"', whitelist_match.group(1)))

    missing = [s for s in scanners if s not in whitelist]

    if missing:
        return UpdateCheckResult("UC-04", name, "error", False,
                                 f"扫描器未加入 build_dist.py runtime_scripts 白名单：{missing}",
                                 f"在 _copy_runtime_scripts_to_dist 的 runtime_scripts 列表追加 {missing}",
                                 details={"scanners": scanners, "whitelist": sorted(whitelist), "missing": missing})

    return UpdateCheckResult("UC-04", name, "error", True,
                             f"扫描器分发一致：{len(scanners)} 个扫描器全部在白名单",
                             details={"scanners": scanners, "whitelist": sorted(whitelist)})


# ─── UC-05 健康度清单覆盖 ────────────────────────────────────────────────────
# ae-sdd-update-skill 健康度清单应包含的关键组件关键词
HEALTH_CHECKLIST_REQUIRED = [
    ("G-RA-1", "G-RA 门禁注册"),
    ("ra_authenticity_scan", "RA 真实性扫描器"),
    ("RequirementAnalysisModel", "RAModel 12 维"),
    ("16 道 RA 质量闸", "16 道 RA-G 闸"),
    ("update_graph", "更新图谱检查器"),
]


def check_uc05_health_checklist(repo_root: Path) -> UpdateCheckResult:
    """UC-05 健康度清单覆盖：ae-sdd-update-skill 健康度清单含本仓库关键组件。"""
    name = "健康度清单覆盖"
    update_skill = repo_root / "source" / "skills" / "orchestration" / "ae-sdd-update-skill.md"

    if not update_skill.is_file():
        return UpdateCheckResult("UC-05", name, "error", False,
                                 "ae-sdd-update-skill.md 不存在")

    text = update_skill.read_text(encoding="utf-8", errors="replace")
    missing = [(kw, desc) for kw, desc in HEALTH_CHECKLIST_REQUIRED if kw not in text]

    if missing:
        return UpdateCheckResult("UC-05", name, "warn", True,
                                 f"健康度清单缺关键组件项：{[desc for _, desc in missing]}",
                                 f"在 ae-sdd-update-skill §子 SKILL 健康度 追加 {missing}",
                                 details={"missing": [desc for _, desc in missing]})

    return UpdateCheckResult("UC-05", name, "warn", True,
                             f"健康度清单覆盖完整：{len(HEALTH_CHECKLIST_REQUIRED)} 个关键组件",
                             details={"covered": [desc for _, desc in HEALTH_CHECKLIST_REQUIRED]})


# ─── 主入口 ──────────────────────────────────────────────────────────────────
CHECK_FUNCS = {
    "UC-01": check_uc01_version,
    "UC-02": check_uc02_gates_registry,
    "UC-03": check_uc03_command_contract,
    "UC-04": check_uc04_scanner_distribution,
    "UC-05": check_uc05_health_checklist,
}


def check_all(repo_root: Optional[Path] = None, only: Optional[str] = None) -> list[UpdateCheckResult]:
    """跑全部更新检查；only 指定时只跑那一个。"""
    root = repo_root or Path.cwd()
    targets = [cid for cid in CHECK_FUNCS if (only is None or cid == only)]
    if only and only not in CHECK_FUNCS:
        return [UpdateCheckResult(only, "未知检查", "error", False,
                                  f"未知检查 ID: {only}（允许: {sorted(CHECK_FUNCS.keys())})")]
    return [CHECK_FUNCS[cid](root) for cid in targets]


def summarize(results: list[UpdateCheckResult]) -> dict:
    """汇总结果（对标 gates.summarize）。"""
    return {
        "total": len(results),
        "passed": sum(1 for r in results if r.pass_),
        "failed": sum(1 for r in results if not r.pass_),
        "warnings": sum(1 for r in results if r.severity == "warn" and r.pass_),
        "all_pass": all(r.pass_ for r in results),
        "checks": [
            {
                "check_id": r.check_id,
                "name": r.name,
                "severity": r.severity,
                "pass": r.pass_,
                "message": r.message,
                "fix": r.fix,
            }
            for r in results
        ],
    }


# ─── 图谱查询 API（v3.2 — Agent 可读可理解，改了文件查连带项）─────────────────
# 权威源是 source/standards/update-graph.json；本节提供 load_graph() 加载 +
# query_affected(changed_files) 查询。Agent 改完文件后调 query_affected 拿到
# "连带项清单 + 该跑哪些 UC-XX 检查"，无需解析 Markdown 表格。

_GRAPH_CACHE: Optional[dict] = None


def load_graph(repo_root: Optional[Path] = None) -> dict:
    """加载 update-graph.json（权威源）。带缓存。"""
    global _GRAPH_CACHE
    if _GRAPH_CACHE is not None:
        return _GRAPH_CACHE
    root = repo_root or Path(__file__).resolve().parent.parent.parent
    graph_path = root / "source" / "standards" / "update-graph.json"
    if not graph_path.is_file():
        raise FileNotFoundError(f"图谱数据文件不存在：{graph_path}")
    import json
    _GRAPH_CACHE = json.loads(graph_path.read_text(encoding="utf-8"))
    return _GRAPH_CACHE


def reload_graph(repo_root: Optional[Path] = None) -> dict:
    """强制重新加载图谱（测试用）。"""
    global _GRAPH_CACHE
    _GRAPH_CACHE = None
    return load_graph(repo_root)


def _match_trigger(changed_file: str, trigger_patterns: list) -> bool:
    """判断单个改动文件是否命中某条规则的 trigger glob 模式。

    支持 ** 多级通配（用 fnmatch 翻译：a/**/b → a/* 匹配任意层级）。
    """
    cf = changed_file.replace("\\", "/")
    for pattern in trigger_patterns:
        p = pattern.replace("\\", "/")
        # ** → 任意层级目录：把 ** 翻译成可匹配多级的通配
        if "**" in p:
            # 转成 fnmatch 友好：**/*.md → 匹配任意层级 .md
            # fnmatch 的 * 不跨 /，这里用分段近似：把 **/ 当作可跨层
            regex = re.escape(p).replace(r"\*\*", ".*").replace(r"\*", "[^/]*")
            if re.fullmatch(regex, cf):
                return True
        elif fnmatch.fnmatch(cf, p):
            return True
    return False


@dataclass
class AffectedQueryResult:
    """query_affected 的返回结构"""
    changed_files: list
    matched_rules: list          # 命中的图谱规则（id/name/trigger_condition）
    affected_items: list         # 去重后的连带项（path/action/auto_checkable/来源规则）
    checks_to_run: list          # 去重后该跑的 UC-XX 检查 ID


def query_affected(changed_files: list, repo_root: Optional[Path] = None) -> AffectedQueryResult:
    """查询：改了这些文件后，连带项是什么 + 该跑哪些检查。

    Agent 改完文件后的标准动作：
      result = query_affected(["tools/lib/gates.py", "source/SKILL.md"])
      → result.affected_items  # 连带项清单
      → result.checks_to_run   # 该跑的 UC-XX
      → 跑 check_all(only=uc) 逐项验证

    Args:
        changed_files: 改动的文件路径列表（相对仓库根，如 "tools/lib/gates.py"）
        repo_root: 仓库根目录
    Returns:
        AffectedQueryResult
    """
    graph = load_graph(repo_root)
    normalized = [f.replace("\\", "/") for f in changed_files]

    matched_rules = []
    affected_seen = set()  # (path, action) 去重
    affected_items = []
    checks_seen = set()
    checks_to_run = []

    for rule in graph.get("rules", []):
        hit = False
        for cf in normalized:
            if _match_trigger(cf, rule.get("trigger", [])):
                hit = True
                break
        if not hit:
            continue

        matched_rules.append({
            "id": rule["id"],
            "name": rule["name"],
            "trigger_condition": rule.get("trigger_condition", ""),
        })

        for aff in rule.get("affected", []):
            key = (aff["path"], aff["action"])
            if key not in affected_seen:
                affected_seen.add(key)
                affected_items.append({
                    "path": aff["path"],
                    "action": aff["action"],
                    "auto_checkable": aff.get("auto_checkable", False),
                    "from_rule": rule["id"],
                })

        for uc in rule.get("checks", []):
            if uc not in checks_seen:
                checks_seen.add(uc)
                checks_to_run.append(uc)

    return AffectedQueryResult(
        changed_files=normalized,
        matched_rules=matched_rules,
        affected_items=affected_items,
        checks_to_run=checks_to_run,
    )
