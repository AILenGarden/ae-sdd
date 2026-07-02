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

    text = update_skill.read_text(encoding="utf-8", errors="replace")
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

    details = {
        "rule_ids": rule_ids,
        "graph_check_ids": graph_check_ids,
        "registered_check_ids": registered_check_ids,
        "missing_rule_ids": missing_rule_ids,
        "missing_check_ids_in_skill": missing_check_ids_in_skill,
        "missing_check_funcs": missing_check_funcs,
        "unreferenced_check_funcs": unreferenced_check_funcs,
        "missing_protocol_terms": missing_protocol_terms,
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
    skill_md = repo_root / "source" / "SKILL.md"
    if skill_md.is_file():
        sk_text = skill_md.read_text(encoding="utf-8", errors="replace")
        if "## 🚀 自动化模式" not in sk_text:
            issues.append("SKILL.md 缺 §🚀 自动化模式章节")
        if "G-AUTO-CONSENSUS" not in sk_text:
            issues.append("SKILL.md 门禁速查缺 G-AUTO-CONSENSUS")
        if "30门禁" not in sk_text and "30 门禁" not in sk_text:
            issues.append("SKILL.md 工具速查门禁数未更新为 30")
    else:
        issues.append("source/SKILL.md 不存在")

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


# ─── 主入口 ──────────────────────────────────────────────────────────────────
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
