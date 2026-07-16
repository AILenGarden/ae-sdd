"""
update_graph.py — ae-sdd 更新依赖图谱检查器（v3.2）

把 ae-sdd-update-skill 的"更新依赖图谱"从纸面规则变成可执行检查。
对标 gates.py 的 GateResult 模式：每项检查产出一个 UpdateCheckResult。

基础检查：
  UC-01 版本号一致性    SKILL.md / paths.py / README.md 三处版本号必须一致
  UC-02 门禁注册一致性  GATE_REGISTRY 每个 id 都在 CHECK_FUNCS 或 check_all 特判中
  UC-03 命令契约闭环    SKILL.md 引用的 `ae-sdd <cmd>` 都在 CLI add_parser 注册
  UC-04 扫描器分发一致性 scripts/*_scan.py 都在 build_dist.py runtime_scripts 白名单
  UC-05 健康度清单覆盖  ae-sdd-update-skill 健康度清单含本仓库关键组件
  UC-14 update-skill 级联图谱同步  update-graph.json / CHECK_FUNCS / 人读视图三方同步
  UC-20 Design Ledger 契约        系统设计问题/价值/证据/版本与 CHANGELOG impact 可发现且覆盖完整

无第三方依赖，可独立运行。
"""
from __future__ import annotations

import fnmatch
import importlib.util
import json
import re
import sys
import tempfile
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
# 注：`assets` 组已实现 query/outline/section/stats/read，且 2026-07-09 补齐
# generate/check baseline 生成与校验；assets update/audit 仍未实现。
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
    ("G-CODEPLAN-SRC", "🆕 v3.4.0 源码核对门禁"),
    ("G-DOC-STORAGE", "🆕 v3.4.0 文档存放门禁"),
    ("G-DOC-CONSISTENCY", "🆕 v3.5.7 项目侧记忆-配置路径一致性门禁"),
    ("UC-06", "🆕 v3.4.0 文档-实现一致性检查"),
    ("UC-14", "🆕 2026-07-02 update-skill 级联图谱同步检查"),
    ("UC-15", "runtime compile consistency check"),
    ("G-AUTO-CONSENSUS", "🆕 v3.8.0 自动化联审共识门禁"),
    ("UC-16", "🆕 v3.8.0 自动化级联一致性检查"),
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


# ─── UC-06 文档-实现一致性（🆕 v3.4.0，建议书3 §7.6）─────────────────────────
# 检测"文档承诺门禁但门禁不存在"的文档撒谎复发：
# (a) source/SKILL.md + 子 SKILL 引用的 ae-sdd <cmd> 都在 CLI 实现（扩展 UC-03 到子 SKILL）
# (b) HARNESS.md 声明的 HS-X 硬停止规则在 gate_intercept/stop_check/prompt_inject 有实现

# HARNESS.md HS 规则 → 实现文件关键词映射（声明但无对应实现 → warn）
_HARNESS_HS_IMPL_MAP = {
    "HS-1": ("gate_intercept.py", "_DESIGN_PHASES"),
    "HS-2": ("gate_intercept.py", "_check_state_write"),
    "HS-3": (None, None),   # 声明但无物理实现（模糊回复）— 标 warn
    "HS-7": ("gate_intercept.py", "prd-complete"),  # 声明，部分实现
    "HS-8": ("stop_check.py", "compact"),           # 声明，部分实现
    "HS-9": ("prompt_inject.py", "AE_SDD_TRIGGER_MARKERS"),
    "HS-10": ("gate_intercept.py", "_PRODUCT_PHASE_MAP"),
    "HS-11": ("gate_intercept.py", "task-reviewed"),
    "HS-12": ("stop_check.py", "_verify_gate_claims"),
}


def check_uc06_doc_impl_consistency(repo_root: Path) -> UpdateCheckResult:
    """UC-06 文档-实现一致性：SKILL/子SKILL 引用命令都在 CLI + HARNESS HS 规则有实现（建议书3 §7.6）。"""
    name = "文档-实现一致性"
    issues: list[str] = []
    warnings: list[str] = []

    # (a) 子 SKILL 引用的命令都在 CLI 实现（扩展 UC-03 到 source/skills/**/*.md）
    cli_cmds = _extract_cli_commands(repo_root / "tools" / "bin" / "ae-sdd")
    skills_dir = repo_root / "source" / "skills"
    if skills_dir.is_dir():
        for skill_md in skills_dir.rglob("*-skill.md"):
            try:
                text = skill_md.read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue
            # 跳过 YAML frontmatter
            body = text
            if text.startswith("---"):
                end = text.find("\n---", 3)
                if end != -1:
                    body = text[end + 4:]
            # 严格匹配实际命令调用：ae-sdd <cmd> 后紧跟 --flag / 反引号闭合 / 行尾
            # 防止 "ae-sdd 生成的文档" 这类正文误匹配（建议书3 §7.6）
            refs = set(re.findall(
                r"ae-sdd\s+([a-z][a-z-]*)(?=\s+--|`|\s*$)", body, re.MULTILINE))
            refs -= {"description", "version", "name"}
            missing = refs - cli_cmds - HISTORICAL_UNIMPLEMENTED - {"v"}
            if missing:
                issues.append(f"{skill_md.relative_to(repo_root)} 引用未实现命令：{sorted(missing)}")

    # (b) HARNESS.md 声明的 HS 规则有实现
    harness = repo_root / "source" / "HARNESS.md"
    if harness.is_file():
        harness_text = harness.read_text(encoding="utf-8", errors="replace")
        declared_hs = set(re.findall(r"\bHS-(\d+)\b", harness_text))
        for hs_num in sorted(declared_hs, key=int):
            hs_id = f"HS-{hs_num}"
            impl_file, impl_kw = _HARNESS_HS_IMPL_MAP.get(hs_id, (None, None))
            if impl_file is None:
                # 无映射的 HS 规则 → 声明但无实现 → warn
                warnings.append(f"{hs_id} 声明但无物理实现（靠 agent 自律 + 兜底机制）")
                continue
            impl_path = repo_root / "tools" / "lib" / impl_file
            if not impl_path.is_file():
                issues.append(f"{hs_id} 声明但实现文件缺失：{impl_file}")
                continue
            try:
                impl_text = impl_path.read_text(encoding="utf-8", errors="replace")
            except OSError:
                issues.append(f"{hs_id} 实现文件不可读：{impl_file}")
                continue
            if impl_kw and impl_kw not in impl_text:
                warnings.append(f"{hs_id} 声明在 {impl_file} 但未找到 '{impl_kw}' 实现")

    if issues:
        return UpdateCheckResult("UC-06", name, "error", False,
                                 f"文档-实现不一致（{len(issues)} 项）：{issues[:5]}",
                                 "实现承诺的命令/HS 规则，或删除文档中的虚假承诺（建议书3 §7.6）",
                                 details={"issues": issues, "warnings": warnings})

    if warnings:
        return UpdateCheckResult("UC-06", name, "warn", True,
                                 f"文档-实现基本一致（{len(warnings)} 项声明但无物理实现，warn）：{warnings[:5]}",
                                 "这些 HS 规则靠 agent 自律 + 兜底机制，可后续补物理实现",
                                 details={"warnings": warnings})

    return UpdateCheckResult("UC-06", name, "error", True,
                             "文档-实现一致：SKILL/子SKILL 命令全实现 + HARNESS HS 规则全有实现",
                             details={"cli_cmds_count": len(cli_cmds)})


def check_uc07_distribution_closure(repo_root: Path) -> UpdateCheckResult:
    """🆕 v3.4.0 UC-07 分发闭环：post-commit hook 必须存在 + git hooksPath 指向它 + hooksPath 与 SKILL.md/HARNESS.md 声明一致。

    v3.4.0 之前的债：母版改了 12 个 commit 没人跑 dev-sync.sh → harness/agent.md 停在 v3.1.2。
    v3.4.0 修复：.githooks/post-commit 强制分发链自动跑。

    验证项：
    1. `.githooks/post-commit` 文件存在 + 可执行
    2. `git config core.hooksPath` = `.githooks`
    3. HARNESS.md 包含"分发闭环"章节 + 引用 .githooks/post-commit
    4. ae-sdd-update-skill.md 包含"v3.4.0 自动分发闭环"章节
    """
    name = "v3.4.0 分发闭环（post-commit hook）"
    issues: list[str] = []
    warnings: list[str] = []

    # 1. .githooks/post-commit 存在 + 可执行
    githooks_post_commit = repo_root / ".githooks" / "post-commit"
    if not githooks_post_commit.is_file():
        issues.append(f".githooks/post-commit 不存在（v3.4.0 必备）")
    else:
        import os as _os_uc07
        if not _os_uc07.access(str(githooks_post_commit), _os_uc07.X_OK):
            issues.append(f".githooks/post-commit 不可执行（chmod +x）")

    # 2. git config core.hooksPath = .githooks
    try:
        import subprocess as _sp_uc07
        r = _sp_uc07.run(
            ["git", "config", "--get", "core.hooksPath"],
            cwd=str(repo_root), capture_output=True, text=True, timeout=5,
        )
        hooks_path = r.stdout.strip() if r.returncode == 0 else ""
        if hooks_path != ".githooks":
            issues.append(
                f"git core.hooksPath={hooks_path or '(默认 .git/hooks)'}，应设为 .githooks "
                f"（跑 bash scripts/install-hooks.sh）"
            )
    except Exception as e:
        warnings.append(f"无法读 git config（{e}）")

    # 3. HARNESS.md 包含"分发闭环"章节
    harness_md = repo_root / "source" / "HARNESS.md"
    if harness_md.is_file():
        text = harness_md.read_text(encoding="utf-8", errors="replace")
        if "分发闭环" not in text or ".githooks/post-commit" not in text:
            issues.append("source/HARNESS.md 缺少'分发闭环'章节或 .githooks/post-commit 引用")
    else:
        warnings.append("source/HARNESS.md 不存在")

    # 4. update-skill 包含"v3.4.0 自动分发闭环"章节
    update_skill = repo_root / "source" / "skills" / "orchestration" / "ae-sdd-update-skill.md"
    if update_skill.is_file():
        text = update_skill.read_text(encoding="utf-8", errors="replace")
        if "v3.4.0 自动分发闭环" not in text and "post-commit" not in text:
            warnings.append("ae-sdd-update-skill.md 缺 v3.4.0 自动分发闭环章节")
    else:
        warnings.append("ae-sdd-update-skill.md 不存在")

    # 5. Mavis harness adapter lock must match the current source inputs.
    harness_agent = repo_root / ".harness" / "agent.md"
    harness_lock = repo_root / ".harness" / ".adapter.lock"
    if not harness_agent.is_file():
        issues.append(".harness/agent.md missing; run python scripts/build_harness.py --source <repo>")
    if not harness_lock.is_file():
        issues.append(".harness/.adapter.lock missing; run python scripts/build_harness.py --source <repo>")
    else:
        try:
            lock_data = json.loads(harness_lock.read_text(encoding="utf-8"))
        except Exception as e:
            issues.append(f".harness/.adapter.lock is unreadable JSON: {e}")
            lock_data = None

        if lock_data is not None:
            build_harness_path = repo_root / "scripts" / "build_harness.py"
            if not build_harness_path.is_file():
                issues.append("scripts/build_harness.py missing; cannot verify .harness/.adapter.lock")
            else:
                try:
                    spec = importlib.util.spec_from_file_location("_ae_sdd_build_harness_for_uc07", build_harness_path)
                    if spec is None or spec.loader is None:
                        raise ImportError(f"cannot load {build_harness_path}")
                    build_harness = importlib.util.module_from_spec(spec)
                    spec.loader.exec_module(build_harness)

                    tpl_agent = repo_root / "scripts" / "templates" / "agent.md.template"
                    tpl_readme = repo_root / "scripts" / "templates" / "README.md.template"
                    expected_source_hash = build_harness.source_input_hash(repo_root, tpl_agent, tpl_readme)
                    expected_template_hash = build_harness.template_hash(tpl_agent)
                    expected_version = build_harness.get_ae_sdd_version(repo_root)
                    expected_adapter = build_harness.ADAPTER_VERSION

                    if lock_data.get("source_input_sha256") != expected_source_hash:
                        issues.append(
                            ".harness/.adapter.lock source_input_sha256 drift: "
                            f"{lock_data.get('source_input_sha256')} != {expected_source_hash}"
                        )
                    if lock_data.get("templateHash") != expected_template_hash:
                        issues.append(
                            ".harness/.adapter.lock templateHash drift: "
                            f"{lock_data.get('templateHash')} != {expected_template_hash}"
                        )
                    if lock_data.get("ae_sdd_version") != expected_version:
                        issues.append(
                            ".harness/.adapter.lock ae_sdd_version drift: "
                            f"{lock_data.get('ae_sdd_version')} != {expected_version}"
                        )
                    if lock_data.get("adapter_version") != expected_adapter:
                        issues.append(
                            ".harness/.adapter.lock adapter_version drift: "
                            f"{lock_data.get('adapter_version')} != {expected_adapter}"
                        )
                except Exception as e:
                    warnings.append(f"cannot verify .harness/.adapter.lock: {e}")

    passed = len(issues) == 0
    if passed:
        msg = "post-commit hook 已装 + hooksPath 正确 + 文档章节齐全 + harness source hash aligned"
    else:
        msg = f"{len(issues)} 项缺失：{'；'.join(issues[:3])}"
    return UpdateCheckResult(
        "UC-07",
        name,
        "error" if not passed else "ok",
        passed,
        msg,
        "跑 bash scripts/install-hooks.sh + 编辑 HARNESS.md / ae-sdd-update-skill.md",
        {"issues_count": len(issues), "warnings_count": len(warnings)},
    )


def _read_update_graph_data(repo_root: Path) -> tuple[Optional[dict], Optional[str]]:
    """读取 update-graph.json；返回 (data, error_message)。"""
    graph_path = repo_root / "source" / "standards" / "update-graph.json"
    if not graph_path.is_file():
        return None, f"图谱数据文件不存在：{graph_path}"
    try:
        return json.loads(graph_path.read_text(encoding="utf-8")), None
    except json.JSONDecodeError as e:
        return None, f"update-graph.json 不是合法 JSON：{e}"


def _graph_rule_and_check_ids(graph: dict) -> tuple[list[str], list[str]]:
    """从图谱提取去重后的 UG/UC ID，排序后稳定输出。"""
    rule_ids = []
    check_ids = []
    for rule in graph.get("rules", []):
        rid = rule.get("id")
        if rid:
            rule_ids.append(rid)
        check_ids.extend(rule.get("checks", []))
    return sorted(set(rule_ids)), sorted(set(check_ids))


def _read_frontmatter_values(text: str) -> dict[str, str]:
    """Read simple YAML frontmatter scalar values used by slim source entries."""
    candidate = text[1:] if text.startswith("\ufeff") else text
    match = re.match(r"^---[ \t]*\r?\n(.*?)\r?\n---[ \t]*(?:\r?\n|$)", candidate, re.DOTALL)
    if not match:
        return {}
    values: dict[str, str] = {}
    for line in match.group(1).splitlines():
        item = re.match(r"^([A-Za-z0-9_-]+):\s*(.*?)\s*$", line)
        if not item:
            continue
        value = item.group(2).strip()
        if (value.startswith('"') and value.endswith('"')) or (value.startswith("'") and value.endswith("'")):
            value = value[1:-1]
        values[item.group(1)] = value
    return values


def _read_update_skill_semantic_text(repo_root: Path, update_skill: Path) -> tuple[str, list[str], list[str]]:
    """Return the update-skill semantic text, including source-slim fallback when present."""
    text = update_skill.read_text(encoding="utf-8", errors="replace")
    texts = [text]
    sources = [update_skill.relative_to(repo_root).as_posix()]
    warnings: list[str] = []

    metadata = _read_frontmatter_values(text)
    if str(metadata.get("source_slimmed", "")).strip().lower() == "true":
        fallback_rel = metadata.get("source_fallback")
        if not fallback_rel:
            warnings.append("ae-sdd-update-skill.md is source_slimmed but has no source_fallback")
        else:
            fallback_path = repo_root / "source" / fallback_rel.replace("\\", "/")
            if fallback_path.is_file():
                texts.append(fallback_path.read_text(encoding="utf-8", errors="replace"))
                sources.append(fallback_path.relative_to(repo_root).as_posix())
            else:
                warnings.append(f"source_fallback missing: source/{fallback_rel}")

    return "\n".join(texts), sources, warnings


def check_uc14_update_skill_cascade_sync(repo_root: Path) -> UpdateCheckResult:
    """UC-14：update-graph.json / CHECK_FUNCS / ae-sdd-update-skill 人读视图三方同步。"""
    name = "update-skill 级联图谱同步"
    graph, error = _read_update_graph_data(repo_root)
    if error:
        return UpdateCheckResult("UC-14", name, "error", False, error,
                                 "修复 source/standards/update-graph.json")

    update_skill = repo_root / "source" / "skills" / "orchestration" / "ae-sdd-update-skill.md"
    if not update_skill.is_file():
        return UpdateCheckResult("UC-14", name, "error", False,
                                 "ae-sdd-update-skill.md 不存在",
                                 "恢复 source/skills/orchestration/ae-sdd-update-skill.md")

    text, semantic_sources, semantic_warnings = _read_update_skill_semantic_text(repo_root, update_skill)
    rule_ids, graph_check_ids = _graph_rule_and_check_ids(graph or {})
    registered_check_ids = sorted(cid for cid in CHECK_FUNCS if re.fullmatch(r"UC-\d+", cid))

    missing_rule_ids = [rid for rid in rule_ids if rid not in text]
    missing_check_ids_in_skill = [cid for cid in graph_check_ids if cid not in text]
    missing_check_funcs = sorted(set(graph_check_ids) - set(registered_check_ids))
    unreferenced_check_funcs = sorted(set(registered_check_ids) - set(graph_check_ids))
    missing_protocol_terms = [
        term for term in (
            "source/standards/update-graph.json",
            "ae-sdd update-check --affected",
            "UC-14",
        )
        if term not in text
    ]

    issues = []
    if missing_rule_ids:
        issues.append(f"update-skill 缺 UG 锚点：{missing_rule_ids}")
    if missing_check_ids_in_skill:
        issues.append(f"update-skill 缺 UC 锚点：{missing_check_ids_in_skill}")
    if missing_check_funcs:
        issues.append(f"图谱引用了未注册检查：{missing_check_funcs}")
    if unreferenced_check_funcs:
        issues.append(f"CHECK_FUNCS 有检查未被图谱引用：{unreferenced_check_funcs}")
    if missing_protocol_terms:
        issues.append(f"update-skill 缺协议关键词：{missing_protocol_terms}")
    if semantic_warnings:
        issues.extend(semantic_warnings)

    details = {
        "rule_ids": rule_ids,
        "graph_check_ids": graph_check_ids,
        "registered_check_ids": registered_check_ids,
        "missing_rule_ids": missing_rule_ids,
        "missing_check_ids_in_skill": missing_check_ids_in_skill,
        "missing_check_funcs": missing_check_funcs,
        "unreferenced_check_funcs": unreferenced_check_funcs,
        "missing_protocol_terms": missing_protocol_terms,
        "semantic_sources": semantic_sources,
        "semantic_warnings": semantic_warnings,
    }
    if issues:
        return UpdateCheckResult(
            "UC-14",
            name,
            "error",
            False,
            "；".join(issues[:3]),
            "同步 source/standards/update-graph.json、tools/lib/update_graph.py:CHECK_FUNCS 与 ae-sdd-update-skill.md §更新依赖图谱锚点",
            details,
        )

    return UpdateCheckResult(
        "UC-14",
        name,
        "error",
        True,
        f"级联图谱同步：{len(rule_ids)} 条 UG / {len(graph_check_ids)} 个 UC / {len(registered_check_ids)} 个注册检查一致",
        details=details,
    )


def _runtime_snapshot(dist: Path) -> dict[str, bytes]:
    paths = [dist / "SKILL.md"]
    runtime_dir = dist / "runtime"
    if runtime_dir.is_dir():
        paths.extend(path for path in runtime_dir.rglob("*") if path.is_file())
    skills_dir = dist / "skills"
    if skills_dir.is_dir():
        paths.extend(path for path in skills_dir.rglob("*.md") if path.is_file())
    return {
        path.relative_to(dist).as_posix(): path.read_bytes()
        for path in sorted(paths)
        if path.is_file()
    }


def check_uc15_runtime_compile_consistency(repo_root: Path) -> UpdateCheckResult:
    """UC-15：runtime 编译一致性。

    在临时 dist 中编译两次，验证 compiled package 结构和字节级幂等；不依赖
    工作区当前 dist 是否刚刚重建，避免阻断"先 update-check 再 build"流程。
    """
    name = "runtime 编译一致性"
    source = repo_root / "source"
    source_skill = source / "SKILL.md"
    compiler = repo_root / "scripts" / "compile_skill_runtime.py"
    if not source_skill.is_file():
        return UpdateCheckResult("UC-15", name, "error", False,
                                 "source/SKILL.md 不存在",
                                 "恢复 source/SKILL.md")
    if not compiler.is_file():
        return UpdateCheckResult("UC-15", name, "error", False,
                                 "scripts/compile_skill_runtime.py 不存在",
                                 "恢复 runtime 编译器")

    scripts_dir = repo_root / "scripts"
    tools_dir = repo_root / "tools"
    inserted: list[str] = []
    for p in (str(scripts_dir), str(tools_dir)):
        if p not in sys.path:
            sys.path.insert(0, p)
            inserted.append(p)
    try:
        from compile_skill_runtime import compile_runtime_package  # type: ignore
        from lib.runtime_verify import verify_runtime_package  # type: ignore
    except Exception as exc:
        return UpdateCheckResult("UC-15", name, "error", False,
                                 f"无法 import runtime 编译/校验模块：{exc}",
                                 "检查 scripts/compile_skill_runtime.py 与 tools/lib/runtime_verify.py 可 import")
    finally:
        for p in inserted:
            if p in sys.path:
                sys.path.remove(p)

    try:
        with tempfile.TemporaryDirectory(prefix="ae-sdd-runtime-uc15-") as td:
            dist = Path(td) / "dist" / "ae-sdd"
            dist.mkdir(parents=True)
            (dist / "SKILL.md").write_text(source_skill.read_text(encoding="utf-8"), encoding="utf-8")

            manifest = compile_runtime_package(
                repo_root,
                source,
                dist,
                build_date="2026-07-02T00:00:00Z",
            )
            verify = verify_runtime_package(dist)
            if not verify.ok:
                return UpdateCheckResult(
                    "UC-15",
                    name,
                    "error",
                    False,
                    f"临时 runtime verify 失败：{verify.issues[:5]}",
                    "修复 runtime 编译器输出或 manifest/load_order/fallback 结构",
                    details={"issues": verify.issues, "warnings": verify.warnings},
                )
            first = _runtime_snapshot(dist)

            compile_runtime_package(
                repo_root,
                source,
                dist,
                build_date="2030-01-01T00:00:00Z",
            )
            second = _runtime_snapshot(dist)
            if first != second:
                changed = sorted(set(first) ^ set(second))
                common_changed = [k for k in sorted(set(first) & set(second)) if first[k] != second[k]]
                return UpdateCheckResult(
                    "UC-15",
                    name,
                    "error",
                    False,
                    f"runtime 编译非幂等：新增/删除 {changed[:5]}，内容变化 {common_changed[:5]}",
                    "移除 runtime 输出中的时间/随机/路径依赖，确保重复编译字节一致",
                    details={"changed_paths": changed, "content_changed": common_changed},
                )

            standalone_script = (
                repo_root
                / "standalone-skills"
                / "skill-runtime-compiler"
                / "scripts"
                / "compile_skill_package.py"
            )
            if not standalone_script.is_file():
                return UpdateCheckResult(
                    "UC-15",
                    name,
                    "error",
                    False,
                    "standalone skill runtime compiler script 缺失",
                    "恢复 standalone-skills/skill-runtime-compiler/scripts/compile_skill_package.py",
                )
            spec = importlib.util.spec_from_file_location(
                "uc15_standalone_skill_runtime_compiler",
                standalone_script,
            )
            if spec is None or spec.loader is None:
                return UpdateCheckResult(
                    "UC-15",
                    name,
                    "error",
                    False,
                    f"无法加载 standalone compiler: {standalone_script}",
                    "检查 standalone compiler 脚本路径与 Python import 兼容性",
                )
            standalone = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(standalone)

            sample = Path(td) / "sample-skill"
            sample.mkdir()
            (sample / "SKILL.md").write_text(
                "---\n"
                "name: sample-skill\n"
                "description: Sample skill for UC-15 standalone compiler idempotence.\n"
                "---\n\n"
                "# Sample Skill\n\n"
                "Use this sample to verify deterministic compilation.\n\n"
                "## Workflow\n\n"
                "Compile twice.\n",
                encoding="utf-8",
            )
            (sample / "references").mkdir()
            (sample / "references" / "guide.md").write_text("# Guide\n", encoding="utf-8")

            standalone_manifest = standalone.compile_skill_package(sample)
            standalone_dist = Path(standalone_manifest["package_path"])
            standalone_first = _runtime_snapshot(standalone_dist)
            standalone.compile_skill_package(sample)
            standalone_second = _runtime_snapshot(standalone_dist)
            if standalone_first != standalone_second:
                changed = sorted(set(standalone_first) ^ set(standalone_second))
                common_changed = [
                    k for k in sorted(set(standalone_first) & set(standalone_second))
                    if standalone_first[k] != standalone_second[k]
                ]
                return UpdateCheckResult(
                    "UC-15",
                    name,
                    "error",
                    False,
                    f"standalone runtime compiler 非幂等：新增/删除 {changed[:5]}，内容变化 {common_changed[:5]}",
                    "移除 standalone compiler runtime 输出中的时间/随机/路径依赖，确保重复编译字节一致",
                    details={"changed_paths": changed, "content_changed": common_changed},
                )

            return UpdateCheckResult(
                "UC-15",
                name,
                "error",
                True,
                f"runtime 编译一致：ae-sdd fingerprint={manifest.get('runtime_fingerprint')} standalone fingerprint={standalone_manifest.get('runtime_fingerprint')}",
                details={
                    "version": manifest.get("version"),
                    "runtime_fingerprint": manifest.get("runtime_fingerprint"),
                    "standalone_runtime_fingerprint": standalone_manifest.get("runtime_fingerprint"),
                    "gate_count": manifest.get("extracts", {}).get("gate_count"),
                    "flow_scales": manifest.get("extracts", {}).get("flow_scales"),
                },
            )
    except Exception as exc:
        return UpdateCheckResult("UC-15", name, "error", False,
                                 f"runtime 编译一致性检查异常：{type(exc).__name__}: {exc}",
                                 "修复 compile_skill_runtime.py / runtime_verify.py 后重跑")


def check_uc16_automation_cascade(repo_root: Path) -> UpdateCheckResult:
    """UC-16：自动化开关级联一致性（🆕 v3.8.0）。

    校验自动化开关相关组件齐备且互相一致：
      1. tools/lib/config.py 存在且含 AUTOMATION_DEFAULTS
      2. tools/lib/gates.py GATE_REGISTRY 含 G-AUTO-CONSENSUS + CHECK_FUNCS 注册
      3. tools/bin/ae-sdd 注册 automation/preflight 子命令
      4. tools/lib/state.py 含 register_review_consensus
      5. scripts/init.py CONFIG_TEMPLATE 含 automation 段
      6. source/SKILL.md 含 §🚀 自动化模式 + 30门禁
    """
    name = "自动化开关级联一致性"
    issues = []

    # 1. config.py
    config_py = repo_root / "tools" / "lib" / "config.py"
    if not config_py.is_file():
        issues.append("tools/lib/config.py 不存在")
    else:
        cfg_text = config_py.read_text(encoding="utf-8", errors="replace")
        if "AUTOMATION_DEFAULTS" not in cfg_text:
            issues.append("config.py 缺 AUTOMATION_DEFAULTS")
        if "is_automation_enabled" not in cfg_text:
            issues.append("config.py 缺 is_automation_enabled")

    # 2. gates.py G-AUTO-CONSENSUS
    gates_py = repo_root / "tools" / "lib" / "gates.py"
    if gates_py.is_file():
        g_text = gates_py.read_text(encoding="utf-8", errors="replace")
        if "G-AUTO-CONSENSUS" not in g_text:
            issues.append("gates.py GATE_REGISTRY 缺 G-AUTO-CONSENSUS")
        if "check_g_auto_consensus" not in g_text:
            issues.append("gates.py 缺 check_g_auto_consensus 实现")
        if '"G-AUTO-CONSENSUS": check_g_auto_consensus' not in g_text:
            issues.append("gates.py CHECK_FUNCS 未注册 G-AUTO-CONSENSUS")
    else:
        issues.append("gates.py 不存在")

    # 3. CLI automation/preflight 子命令
    cli = repo_root / "tools" / "bin" / "ae-sdd"
    if cli.is_file():
        c_text = cli.read_text(encoding="utf-8", errors="replace")
        for sub in ("cmd_automation_status", "cmd_automation_enable",
                    "cmd_automation_disable", "cmd_preflight_collect",
                    "cmd_state_register_review_consensus"):
            if sub not in c_text:
                issues.append(f"CLI 缺 {sub}")
        if 'add_parser("automation"' not in c_text:
            issues.append("CLI 未注册 automation 子命令组")
        if 'add_parser("preflight"' not in c_text:
            issues.append("CLI 未注册 preflight 子命令组")
    else:
        issues.append("tools/bin/ae-sdd 不存在")

    # 4. state.py register_review_consensus
    state_py = repo_root / "tools" / "lib" / "state.py"
    if state_py.is_file():
        s_text = state_py.read_text(encoding="utf-8", errors="replace")
        if "register_review_consensus" not in s_text:
            issues.append("state.py 缺 register_review_consensus")
    else:
        issues.append("state.py 不存在")

    # 5. init.py CONFIG_TEMPLATE automation 段
    init_py = repo_root / "scripts" / "init.py"
    if init_py.is_file():
        i_text = init_py.read_text(encoding="utf-8", errors="replace")
        if "automation:" not in i_text or "enabled: false" not in i_text:
            issues.append("init.py CONFIG_TEMPLATE 缺 automation 段（默认 enabled:false）")
    else:
        issues.append("scripts/init.py 不存在")

    # 6. SKILL.md 自动化模式 + 30门禁
    # 🆕 2026-07-03(B6): 适配 source-slim 架构。source/SKILL.md 可能是 slim entry，
    # 完整章节在 skill-fallbacks/SKILL.full.md。检查时两处都看：slim entry 的语义
    # 清单指针 + fallback 的完整章节。只要任一含目标即视为一致。
    skill_md = repo_root / "source" / "SKILL.md"
    fallback_md = repo_root / "source" / "skill-fallbacks" / "SKILL.full.md"
    sk_text = ""
    fallback_text = ""
    if skill_md.is_file():
        sk_text = skill_md.read_text(encoding="utf-8", errors="replace")
    else:
        issues.append("source/SKILL.md 不存在")
    if fallback_md.is_file():
        fallback_text = fallback_md.read_text(encoding="utf-8", errors="replace")
    combined = sk_text + "\n" + fallback_text
    if "## 🚀 自动化模式" not in combined:
        issues.append("SKILL.md/slim fallback 缺 §🚀 自动化模式章节")
    if "G-AUTO-CONSENSUS" not in sk_text:
        issues.append("SKILL.md 门禁速查缺 G-AUTO-CONSENSUS")
    if "30门禁" not in sk_text and "30 门禁" not in sk_text and "30门禁" not in fallback_text and "30 门禁" not in fallback_text:
        issues.append("SKILL.md 工具速查门禁数未更新为 30")

    if issues:
        return UpdateCheckResult(
            "UC-16", name, "error", False,
            "自动化级联不一致：" + "；".join(issues[:4]),
            "按 UG-20 affected 逐项同步 automation 开关相关组件",
            details={"issues": issues},
        )
    return UpdateCheckResult(
        "UC-16", name, "error", True,
        "自动化开关级联一致：config.py/gates/state/CLI/init/SKILL 六处齐备",
        details={"checked": ["config.py", "gates.py", "state.py", "CLI", "init.py", "SKILL.md"]},
    )


def check_uc17_repo_layout_contract(repo_root: Path) -> UpdateCheckResult:
    """UC-17：仓库顶层结构契约（🆕 v3.9.19）。

    保证「发版包只装 ae-sdd 本体」这条核心纪律不被顶层杂物侵蚀。两层校验：

      1. 顶层无 scratch 残留 —— 下列路径一旦出现在仓库顶层即 FAIL：
         nul / _tmp_*.py / README.docx / update-doc/ / logs/
         （这些是历史遗留的 Windows 重定向误产物、临时脚本、过期归档目录。）
      2. README.md §📦 仓库结构 标注发版包边界 —— 断言 README.md 含
         "发版包边界" 声明，保证结构树与实际取材规则（build_dist.py）同步、
         不因顶层目录增删而脱节。

    本检查是 update-graph UG-23 的机器可读守门人：任何动顶层结构的改动都会
    被 ae-sdd-update-skill 经 update-graph.json 命中，提示跑 UC-17 验证。
    """
    name = "仓库顶层结构契约"
    issues: list[str] = []

    # 1. 顶层 scratch 残留（真实文件系统条目存在即 FAIL）
    #    注意：Windows 上 os.path.exists("nul") 永远返回 True（NUL 是保留设备名，
    #    非真实文件），故改用 os.listdir 的真实条目集合判定，避免跨平台误报。
    import os as _os_uc17
    real_entries = set(_os_uc17.listdir(repo_root))
    scratch_names = {
        "nul", "_tmp_mark1.py", "_tmp_mark2.py", "_tmp_norm.py",
        "README.docx", "update-doc", "logs",
    }
    leaked = sorted(scratch_names & real_entries)
    # _tmp_*.py 用 glob 兜底（防止未来新增的临时脚本漏网）
    for p in repo_root.glob("_tmp_*.py"):
        rel = str(p.relative_to(repo_root)).replace("\\", "/")
        if rel not in leaked:
            leaked.append(rel)
    if leaked:
        issues.append(
            f"顶层存在 scratch 残留：{leaked}；"
            f"这些不进发版包但污染仓库观感，应删除（README.docx 已归档为 README.md）"
        )

    # 2. README.md 仓库结构标注发版包边界
    readme = repo_root / "README.md"
    if not readme.is_file():
        issues.append("README.md 不存在，无法校验仓库结构树")
    else:
        readme_text = readme.read_text(encoding="utf-8", errors="replace")
        if "发版包边界" not in readme_text:
            issues.append(
                "README.md §📦 仓库结构 缺「发版包边界」声明；"
                "顶层结构变更后必须同步该声明（见 RELEASING.md §2.2）"
            )

    if issues:
        return UpdateCheckResult(
            "UC-17", name, "error", False,
            "顶层结构契约违反：" + "；".join(issues),
            "删除 scratch 残留 + 在 README §📦 仓库结构补「发版包边界」声明；详见 RELEASING.md §5",
            details={"issues": issues},
        )
    return UpdateCheckResult(
        "UC-17", name, "error", True,
        "顶层结构契约满足：无 scratch 残留 + README 已标注发版包边界",
        details={"checked": ["top-level scratch", "README 发版包边界声明"]},
    )


def check_uc18_manifest_index_contract(repo_root: Path) -> UpdateCheckResult:
    """UC-18：manifest-index 契约（🆕 v3.9.20）。

    保证 dist 编译后存在 LLM-facing 精简 manifest-index.json，且只含白名单字段
    （防回归——谁要是不小心把 sha256/checksums/generated_files 塞回 index，
    token 膨胀会复发）。两项校验：

      1. dist/ae-sdd/runtime/manifest-index.json 存在 + schema = ae-sdd-runtime-index/v1
      2. index 的 subskills 每条只含白名单字段（entry/manifest/boot/core/outline/fallback），
         不得出现 sha256/fingerprint/checksums 等机器字段
    """
    name = "manifest-index 契约"
    index_path = repo_root / "dist" / "ae-sdd" / "runtime" / "manifest-index.json"
    issues: list[str] = []

    if not index_path.is_file():
        # dist 可能未构建；仅 warn 不阻断（dist 是 gitignored 产物）
        return UpdateCheckResult("UC-18", name, "warn", True,
                                 "dist/ae-sdd/runtime/manifest-index.json 不存在（dist 未构建则跳过；构建后应有）",
                                 details={"skipped": True})

    try:
        idx = json.loads(index_path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as e:
        return UpdateCheckResult("UC-18", name, "error", False,
                                 f"manifest-index.json 不可读：{e}",
                                 "重新跑 scripts/build-dist.sh 生成")

    # 1. schema 校验
    if idx.get("schema") != "ae-sdd-runtime-index/v1":
        issues.append(f"schema 应为 ae-sdd-runtime-index/v1，实为 {idx.get('schema')!r}")

    # 2. subskills 字段白名单（防回归塞回哈希）
    # 🆕 v3.10.3: 新增 core（可执行快路径路由字段，非哈希/校验字段）
    allowed_subskill_keys = {"entry", "manifest", "boot", "core", "outline", "fallback"}
    subskills = idx.get("subskills", [])
    if not isinstance(subskills, list) or not subskills:
        issues.append("subskills 为空或非 list")
    else:
        bad = []
        for i, rec in enumerate(subskills):
            if not isinstance(rec, dict):
                bad.append(f"subskills[{i}] 非 dict")
                continue
            extra = set(rec.keys()) - allowed_subskill_keys
            if extra:
                bad.append(f"subskills[{i}] 含非白名单字段：{sorted(extra)}")
        if bad:
            issues.append("subskills 字段越界（不得含哈希/校验字段）：" + "；".join(bad[:3]))

    # 注：顶层 runtime_fingerprint 是合法路由字段（LLM 需知 runtime 版本指纹），
    # 不在 forbidden 名单；只有 subskills 内部不得出现 sha256/fingerprint/checksums。

    if issues:
        return UpdateCheckResult("UC-18", name, "error", False,
                                 "manifest-index 契约违反：" + "；".join(issues),
                                 "修正 scripts/compile_skill_runtime.py 的 manifest_index 生成，"
                                 "确保 index 只含路由字段（entry/load_order/subskill 路径）",
                                 details={"issues": issues})
    size = index_path.stat().st_size
    return UpdateCheckResult("UC-18", name, "error", True,
                             f"manifest-index 契约满足：schema 正确 + 字段白名单 + {size} bytes",
                             details={"size": size, "subskill_count": len(subskills)})


# ─── 主入口 ──────────────────────────────────────────────────────────────────
def check_uc19_operation_maintenance_contract(repo_root: Path) -> UpdateCheckResult:
    """UC-19: keep typed-operation maintenance instructions executable and discoverable."""
    name = "typed operation maintenance contract"
    root = Path(repo_root)
    version = _extract_skill_version(root / "source" / "SKILL.md")
    protocol_path = root / "source" / "standards" / "operation-protocol.md"
    update_skill = root / "source" / "skills" / "orchestration" / "ae-sdd-update-skill.md"
    design_path = root / "source" / "docs" / "ae-sdd-design.md"
    architecture_path = root / "source" / "docs" / "ae-sdd-implementation-architecture.md"
    operations_path = root / "tools" / "lib" / "operations.py"

    protocol_text = protocol_path.read_text(encoding="utf-8", errors="replace") if protocol_path.is_file() else ""
    if update_skill.is_file():
        update_text, semantic_sources, semantic_warnings = _read_update_skill_semantic_text(root, update_skill)
    else:
        update_text, semantic_sources, semantic_warnings = "", [], []
    design_text = design_path.read_text(encoding="utf-8", errors="replace") if design_path.is_file() else ""
    architecture_text = architecture_path.read_text(encoding="utf-8", errors="replace") if architecture_path.is_file() else ""
    operations_text = operations_path.read_text(encoding="utf-8", errors="replace") if operations_path.is_file() else ""

    protocol_anchors = [
        "## 9. Maintainer Change Contract",
        "### 9.1 Authority And Truth",
        "### 9.2 Compatibility Classification",
        "### 9.3 Operation Admission",
        "### 9.4 Required Change Set",
        "### 9.5 Definition Of Done",
    ]
    missing_protocol_anchors = [anchor for anchor in protocol_anchors if anchor not in protocol_text]
    update_terms = [
        "source/standards/operation-protocol.md",
        "ops describe --json",
        "registryVersion",
        "UG-27",
        "UC-19",
        "Maintainer Change Contract",
    ]
    missing_update_terms = [term for term in update_terms if term not in update_text]

    graph, graph_error = _read_update_graph_data(root)
    ug27 = next((rule for rule in (graph or {}).get("rules", []) if rule.get("id") == "UG-27"), None)
    required_triggers = [
        "tools/lib/state_store.py",
        "tools/lib/operations.py",
        "tools/lib/update_graph.py",
        "source/standards/operation-protocol.md",
        "source/skill-fallbacks/skills/orchestration/ae-sdd-update-skill.full.md",
        "source/docs/ae-sdd-design.md",
        "source/docs/ae-sdd-implementation-architecture.md",
    ]
    required_affected = [
        "tools/lib/update_graph.py",
        "tools/tests/test_update_graph.py",
        "source/standards/operation-protocol.md",
        "source/skills/orchestration/ae-sdd-update-skill.md",
        "source/skill-fallbacks/skills/orchestration/ae-sdd-update-skill.full.md",
        "source/docs/ae-sdd-design.md",
        "source/docs/ae-sdd-implementation-architecture.md",
        "source/CHANGELOG/",
    ]
    graph_triggers = list((ug27 or {}).get("trigger") or [])
    graph_affected = [str(item.get("path")) for item in ((ug27 or {}).get("affected") or []) if isinstance(item, dict)]
    missing_ug27_paths = [
        *[f"trigger:{path}" for path in required_triggers if path not in graph_triggers],
        *[f"affected:{path}" for path in required_affected if path not in graph_affected],
    ]

    missing_files = [
        path for path, present in {
            "source/SKILL.md": version is not None,
            "source/standards/operation-protocol.md": protocol_path.is_file(),
            "source/skills/orchestration/ae-sdd-update-skill.md": update_skill.is_file(),
            "source/docs/ae-sdd-design.md": design_path.is_file(),
            "source/docs/ae-sdd-implementation-architecture.md": architecture_path.is_file(),
            "tools/lib/operations.py": operations_path.is_file(),
        }.items() if not present
    ]
    version_drift = []
    if version:
        for path, text in (
            ("source/docs/ae-sdd-design.md", design_text),
            ("source/docs/ae-sdd-implementation-architecture.md", architecture_text),
        ):
            if f"v{version}" not in "\n".join(text.splitlines()[:10]):
                version_drift.append(path)
    missing_registry_symbols = [
        symbol for symbol in ("SCHEMA_VERSION", "REGISTRY_VERSION") if symbol not in operations_text
    ]

    issues = []
    if graph_error:
        issues.append(graph_error)
    if missing_files:
        issues.append(f"missing files/version: {missing_files}")
    if missing_protocol_anchors:
        issues.append(f"protocol anchors missing: {missing_protocol_anchors}")
    if missing_update_terms:
        issues.append(f"update-skill anchors missing: {missing_update_terms}")
    if missing_ug27_paths:
        issues.append(f"UG-27 cascade missing: {missing_ug27_paths}")
    if version_drift:
        issues.append(f"design/implementation version drift: {version_drift}")
    if missing_registry_symbols:
        issues.append(f"registry version symbols missing: {missing_registry_symbols}")
    if semantic_warnings:
        issues.extend(semantic_warnings)

    details = {
        "version": version,
        "semantic_sources": semantic_sources,
        "missing_files": missing_files,
        "missing_protocol_anchors": missing_protocol_anchors,
        "missing_update_terms": missing_update_terms,
        "missing_ug27_paths": missing_ug27_paths,
        "version_drift": version_drift,
        "missing_registry_symbols": missing_registry_symbols,
    }
    if issues:
        return UpdateCheckResult(
            "UC-19", name, "error", False,
            "; ".join(issues[:4]),
            "sync operation-protocol, ae-sdd-update, UG-27, design docs and registry version markers",
            details,
        )
    return UpdateCheckResult(
        "UC-19", name, "error", True,
        "typed operation maintenance contract is aligned",
        details=details,
    )


# ─── UC-20 Design Ledger contract ───────────────────────────────────────────
DESIGN_LEDGER_IDS = tuple(f"D-{index:03d}" for index in range(1, 26))


def check_uc20_design_ledger(repo_root: Path) -> UpdateCheckResult:
    """UC-20: keep design problems, value hypotheses and iteration impact discoverable."""
    name = "Design Ledger 问题、价值与迭代记录契约"
    root = Path(repo_root)
    design_path = root / "source" / "docs" / "ae-sdd-design.md"
    architecture_path = root / "source" / "docs" / "ae-sdd-implementation-architecture.md"
    changelog_template_path = root / "source" / "CHANGELOG" / "_template.md"
    fallback_update_path = root / "source" / "skill-fallbacks" / "skills" / "orchestration" / "ae-sdd-update-skill.full.md"
    slim_update_path = root / "source" / "skills" / "orchestration" / "ae-sdd-update-skill.md"
    graph_path = root / "source" / "standards" / "update-graph.json"

    def read(path: Path) -> str:
        return path.read_text(encoding="utf-8", errors="replace") if path.is_file() else ""

    design_text = read(design_path)
    architecture_text = read(architecture_path)
    changelog_text = read(changelog_template_path)
    update_text = read(fallback_update_path) + "\n" + read(slim_update_path)
    version = _extract_skill_version(root / "source" / "SKILL.md")

    required_files = {
        "source/docs/ae-sdd-design.md": design_path.is_file(),
        "source/docs/ae-sdd-implementation-architecture.md": architecture_path.is_file(),
        "source/CHANGELOG/_template.md": changelog_template_path.is_file(),
        "source/skill-fallbacks/skills/orchestration/ae-sdd-update-skill.full.md": fallback_update_path.is_file(),
        "source/skills/orchestration/ae-sdd-update-skill.md": slim_update_path.is_file(),
        "source/standards/update-graph.json": graph_path.is_file(),
    }
    missing_files = [path for path, present in required_files.items() if not present]

    ledger_heading = "## 0. 设计问题与价值总览（Design Ledger）"
    required_columns = [
        "| ID |", "要解决的问题", "核心决策", "预期价值",
        "验证证据/指标", "权威入口", "引入/最近变更/状态",
    ]
    missing_terms = [term for term in [ledger_heading, *required_columns] if term not in design_text]

    rows: dict[str, str] = {}
    invalid_rows: list[str] = []
    for line in design_text.splitlines():
        match = re.match(r"^\|\s*(D-\d{3})\s*\|", line)
        if not match:
            continue
        design_id = match.group(1)
        if design_id in rows:
            invalid_rows.append(f"duplicate:{design_id}")
        rows[design_id] = line
        fields = [field.strip() for field in line.strip().strip("|").split("|")]
        if len(fields) < 8:
            invalid_rows.append(f"columns:{design_id}")
            continue
        if any(not field for field in fields[1:8]):
            invalid_rows.append(f"empty:{design_id}")
        if any(token in line for token in ("TODO", "TBD", "<占位>", "<待补>")):
            invalid_rows.append(f"placeholder:{design_id}")

    missing_design_ids = [design_id for design_id in DESIGN_LEDGER_IDS if design_id not in rows]
    unknown_design_ids = sorted(set(rows) - set(DESIGN_LEDGER_IDS))
    section_mappings: set[int] = set()
    for line in rows.values():
        section_mappings.update(int(value) for value in re.findall(r"§(\d+)", line))
    section_headings = {
        int(value) for value in re.findall(r"^##\s+(\d+)\.\s+", design_text, re.MULTILINE)
    }
    missing_section_mappings = [
        f"§{index}" for index in range(1, 23)
        if index not in section_mappings or index not in section_headings
    ]

    typed_operation_terms = ["LLM", "上下文", "命令猜测", "重试"]
    missing_typed_operation_terms = [term for term in typed_operation_terms if term not in rows.get("D-007", "")]
    required_update_terms = [
        "Design Ledger Maintenance Entry",
        "source/docs/ae-sdd-design.md",
        "Design ledger impact",
        "UG-28",
        "UC-20",
        "待补基线",
    ]
    missing_update_terms = [term for term in required_update_terms if term not in update_text]
    required_architecture_terms = ["Design Ledger", "Design ledger impact", "UC-20"]
    missing_architecture_terms = [term for term in required_architecture_terms if term not in architecture_text]
    required_changelog_terms = ["Design ledger impact", "D-xxx", "N/A: no design semantics changed"]
    missing_changelog_terms = [term for term in required_changelog_terms if term not in changelog_text]

    graph, graph_error = _read_update_graph_data(root)
    ug28 = next((rule for rule in (graph or {}).get("rules", []) if rule.get("id") == "UG-28"), None)
    required_graph_paths = [
        "source/docs/ae-sdd-design.md",
        "source/CHANGELOG/_template.md",
        "tools/lib/update_graph.py",
        "tools/tests/test_update_graph.py",
    ]
    graph_triggers = list((ug28 or {}).get("trigger") or [])
    graph_affected = [
        str(item.get("path")) for item in ((ug28 or {}).get("affected") or [])
        if isinstance(item, dict)
    ]
    missing_graph_paths = [
        *[f"trigger:{path}" for path in required_graph_paths if path not in graph_triggers],
        *[f"affected:{path}" for path in required_graph_paths if path not in graph_affected],
    ]
    if not ug28:
        missing_graph_paths.append("UG-28")
    elif "UC-20" not in list(ug28.get("checks") or []):
        missing_graph_paths.append("checks:UC-20")

    version_drift: list[str] = []
    if version:
        for path, text in (
            ("source/docs/ae-sdd-design.md", design_text),
            ("source/docs/ae-sdd-implementation-architecture.md", architecture_text),
        ):
            if f"v{version}" not in "\n".join(text.splitlines()[:12]):
                version_drift.append(path)

    issues: list[str] = []
    if missing_files:
        issues.append(f"missing files: {missing_files}")
    if missing_terms:
        issues.append(f"ledger anchors missing: {missing_terms}")
    if missing_design_ids:
        issues.append(f"design IDs missing: {missing_design_ids}")
    if unknown_design_ids:
        issues.append(f"unknown design IDs: {unknown_design_ids}")
    if invalid_rows:
        issues.append(f"invalid ledger rows: {invalid_rows}")
    if missing_section_mappings:
        issues.append(f"section mappings missing: {missing_section_mappings}")
    if missing_typed_operation_terms:
        issues.append(f"D-007 terms missing: {missing_typed_operation_terms}")
    if missing_update_terms:
        issues.append(f"update-skill terms missing: {missing_update_terms}")
    if missing_architecture_terms:
        issues.append(f"architecture terms missing: {missing_architecture_terms}")
    if missing_changelog_terms:
        issues.append(f"changelog terms missing: {missing_changelog_terms}")
    if missing_graph_paths:
        issues.append(f"UG-28 cascade missing: {missing_graph_paths}")
    if version_drift:
        issues.append(f"design/implementation version drift: {version_drift}")
    if graph_error:
        issues.append(graph_error)

    details = {
        "version": version,
        "design_ids": sorted(rows),
        "missing_design_ids": missing_design_ids,
        "unknown_design_ids": unknown_design_ids,
        "invalid_rows": invalid_rows,
        "missing_section_mappings": missing_section_mappings,
        "missing_terms": missing_terms + missing_changelog_terms,
        "missing_typed_operation_terms": missing_typed_operation_terms,
        "missing_update_terms": missing_update_terms,
        "missing_architecture_terms": missing_architecture_terms,
        "missing_changelog_terms": missing_changelog_terms,
        "missing_graph_paths": missing_graph_paths,
        "version_drift": version_drift,
    }
    if issues:
        return UpdateCheckResult(
            "UC-20", name, "error", False,
            "; ".join(issues[:4]),
            "sync the Design Ledger, changelog impact field, ae-sdd-update entry and UG-28/UC-20",
            details,
        )
    return UpdateCheckResult(
        "UC-20", name, "error", True,
        f"Design Ledger aligned: {len(rows)} designs cover §1~§22 plus cross-cutting governance",
        details=details,
    )


CHECK_FUNCS = {
    "UC-01": check_uc01_version,
    "UC-02": check_uc02_gates_registry,
    "UC-03": check_uc03_command_contract,
    "UC-04": check_uc04_scanner_distribution,
    "UC-05": check_uc05_health_checklist,
    "UC-06": check_uc06_doc_impl_consistency,
    "UC-07": check_uc07_distribution_closure,
    "UC-14": check_uc14_update_skill_cascade_sync,
    "UC-15": check_uc15_runtime_compile_consistency,
    "UC-16": check_uc16_automation_cascade,
    "UC-17": check_uc17_repo_layout_contract,
    "UC-18": check_uc18_manifest_index_contract,
    "UC-19": check_uc19_operation_maintenance_contract,
    "UC-20": check_uc20_design_ledger,
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


# ─── 🆕 v3.8.1 S-4：规则-工具同步 manifest（health 第 10 项依赖） ──────────────
# 治 S-4 缺口：health 原 item 9 master-freshness 只比版本号字符串，无法检测
# "同版本内规则-代码漂移"（如 SKILL.md 声明了 gate 但 gates.py 未实装）。
# 本节提供 manifest 生成 + 漂移检测：build_dist 时生成 .sync-manifest.json，
# health 读 manifest 比对当前文件 hash，漂移则 warn（不阻断，与 item 9 同级）。
import hashlib as _hashlib

SYNC_MANIFEST_FILENAME = ".sync-manifest.json"
SYNC_MANIFEST_GENERATOR_VERSION = "1.0"


def _sha256_of_file(path: Path) -> Optional[str]:
    """计算文件 sha256；不存在或读失败返回 None。"""
    if not path.is_file():
        return None
    try:
        h = _hashlib.sha256()
        h.update(path.read_bytes())
        return h.hexdigest()
    except OSError:
        return None


def sync_manifest_path(repo_root: Path) -> Path:
    """manifest 落地路径：tools/.sync-manifest.json（与 tools/ 同目录便于分发）。"""
    return repo_root / "tools" / SYNC_MANIFEST_FILENAME


def generate_sync_manifest(repo_root: Path) -> dict:
    """生成规则-工具同步 manifest（供 build_dist 调用）。

    读取 update-graph.json 的每条 UG 规则，记录 trigger 文件 + 所有 affected 文件的
    sha256。生成后由调用方写入 sync_manifest_path(repo_root)。

    Returns:
        manifest dict（可 json.dump）
    """
    graph, error = _read_update_graph_data(repo_root)
    if error:
        return {"error": error, "generatedAt": _graph_now_iso()}

    rules_snapshot = []
    for rule in (graph or {}).get("rules", []):
        rule_id = rule.get("id", "")
        name = rule.get("name", "")
        triggers = rule.get("trigger", []) or []
        affected = rule.get("affected", []) or []

        trigger_files = []
        for trig in triggers:
            # trigger 支持 glob 模式（如 "source/skills/**"），展开为实际文件
            for resolved in _resolve_trigger_paths(repo_root, trig):
                sha = _sha256_of_file(resolved)
                if sha is not None:
                    trigger_files.append({
                        "path": _rel_path(resolved, repo_root),
                        "sha256": sha,
                    })

        affected_files = []
        for aff in affected:
            aff_path = aff.get("path", "")
            resolved = repo_root / aff_path
            sha = _sha256_of_file(resolved)
            if sha is not None:
                affected_files.append({
                    "path": aff_path,
                    "sha256": sha,
                    "auto_checkable": bool(aff.get("auto_checkable", False)),
                })

        rules_snapshot.append({
            "id": rule_id,
            "name": name,
            "trigger_files": trigger_files,
            "affected_files": affected_files,
        })

    return {
        "generatedAt": _graph_now_iso(),
        "generatorVersion": SYNC_MANIFEST_GENERATOR_VERSION,
        "rules": rules_snapshot,
    }


def write_sync_manifest(repo_root: Path) -> Path:
    """生成并写入 sync manifest，返回落地路径。"""
    manifest = generate_sync_manifest(repo_root)
    out_path = sync_manifest_path(repo_root)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    return out_path


def check_sync_drift(repo_root: Path) -> dict:
    """比对当前文件 hash 与 manifest 记录，返回漂移报告（供 health 第 10 项调用）。

    Returns:
        {
          "manifest_exists": bool,
          "generatedAt": str,
          "total_rules": int,
          "drifted_rules": [{"id","name","drifted_files":[{"path","kind"}]}],
          "drift_count": int,
        }
        manifest 缺失时 manifest_exists=False，其余字段为零值。
    """
    out_path = sync_manifest_path(repo_root)
    if not out_path.is_file():
        return {"manifest_exists": False, "generatedAt": "", "total_rules": 0,
                "drifted_rules": [], "drift_count": 0}
    try:
        manifest = json.loads(out_path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return {"manifest_exists": False, "generatedAt": "", "total_rules": 0,
                "drifted_rules": [], "drift_count": 0}

    drifted_rules = []
    for rule in manifest.get("rules", []):
        drifted_files = []
        # 拆成两个显式循环追踪来源，避免合并列表后用 `in` 判成员导致
        # 同内容 dict 被误标为 trigger（B2 修复：kind 必须反映真实来源）。
        for entry in rule.get("trigger_files", []):
            cur_sha = _sha256_of_file(repo_root / entry["path"])
            if cur_sha != entry.get("sha256"):
                drifted_files.append({"path": entry["path"], "kind": "trigger"})
        for entry in rule.get("affected_files", []):
            cur_sha = _sha256_of_file(repo_root / entry["path"])
            if cur_sha != entry.get("sha256"):
                drifted_files.append({"path": entry["path"], "kind": "affected"})
        if drifted_files:
            drifted_rules.append({
                "id": rule.get("id", ""),
                "name": rule.get("name", ""),
                "drifted_files": drifted_files,
            })

    return {
        "manifest_exists": True,
        "generatedAt": manifest.get("generatedAt", ""),
        "total_rules": len(manifest.get("rules", [])),
        "drifted_rules": drifted_rules,
        "drift_count": len(drifted_rules),
    }


def _graph_now_iso() -> str:
    from datetime import datetime, timezone
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _resolve_trigger_paths(repo_root: Path, trigger: str) -> list[Path]:
    """把 trigger 模式（含 glob 如 source/skills/**）展开为实际文件路径。

    对非 glob 路径直接返回单元素列表（文件存在与否由调用方 _sha256_of_file 判定）。
    """
    if "*" not in trigger and "?" not in trigger:
        return [repo_root / trigger]
    # glob 展开（相对 repo_root）
    return sorted((repo_root).glob(trigger))


def _rel_path(path: Path, root: Path) -> str:
    try:
        return str(path.relative_to(root)).replace("\\", "/")
    except ValueError:
        return str(path).replace("\\", "/")

