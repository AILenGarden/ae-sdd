"""
iteration_check.py — 设计-实现一致性迭代检查器（🆕 v3.5.4）

把 ae-sdd-update-skill 的"设计-实现一致性迭代检查"从纯人工 SOP 硬化为可执行 CLI。
补 UC-01~07 自动检查的 4 类盲区：
  IC-1 过时技术栈/幽灵命令扫描  SKILL.md 引用的命令/机制是否与 CLI 实际注册/技术栈匹配
  IC-2 F-1 交叉验证覆盖面        stop_check.py 的 GATE 交叉验证覆盖几个 gate；若已废弃自报检测则确认诚实降级
  IC-3 已实现未接入扫描          tools/lib/*.py 实现了但无人 import + untracked 模块
  IC-4 HS 物理实现粗筛           HARNESS.md HS-N 声明 vs 三 hook 文件实际实现

定位：UC 的"深挖层"，不阻断 dev-sync（report-only）；与 update_check 互补不替代。
对应 SOP：source/skills/orchestration/ae-sdd-update-skill.md §设计-实现一致性迭代检查
"""
from __future__ import annotations

import re
import subprocess
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional


@dataclass
class IterationFinding:
    """单项发现"""
    check_id: str          # IC-1 ~ IC-4
    severity: str          # "warn" | "info"
    item: str              # 发现项简述
    location: str          # file:line 或定位
    detail: str            # 详情


@dataclass
class IterationReport:
    """迭代检查报告（不阻断，report-only）"""
    findings: list[IterationFinding] = field(default_factory=list)
    checks_run: list[str] = field(default_factory=list)

    @property
    def n_warn(self) -> int:
        return sum(1 for f in self.findings if f.severity == "warn")

    @property
    def n_info(self) -> int:
        return sum(1 for f in self.findings if f.severity == "info")

    def to_dict(self) -> dict:
        return {
            "checks_run": self.checks_run,
            "n_findings": len(self.findings),
            "n_warn": self.n_warn,
            "n_info": self.n_info,
            "findings": [
                {
                    "check_id": f.check_id, "severity": f.severity,
                    "item": f.item, "location": f.location, "detail": f.detail,
                }
                for f in self.findings
            ],
        }


# ─── IC-1 过时技术栈/幽灵命令扫描 ─────────────────────────────────────────────
# v3.0 残留的过时技术栈关键词（本仓是 Python CLI，无 rules.yaml/.mjs/sync-tools）
_OBSOLETE_TECH_KEYWORDS: tuple[str, ...] = (
    "rules.yaml", "sync-tools", "sync_tools",
    "tools/lib/*.mjs", "tools/schemas/*.json", "tools/tests/*.test.mjs",
    "Node.js ESM",
)
# 已知幽灵命令（CLI 未注册但 SKILL.md 可能仍引用）
# 注：assets check/generate/update/audit 已在 v3.5.4 SKILL.md 修订中删除，
# 此处仍扫描以防回退；run/quick/init/fork/skill 是 UC-03 已标的历史遗留
_GHOST_COMMANDS: tuple[str, ...] = (
    "assets check", "assets generate", "assets update", "assets audit",
    "sync-tools", "state validate", "state show", "state diff", "state lock",
)


def _extract_cli_commands(cli_path: Path) -> set[str]:
    """从 tools/bin/ae-sdd 的 add_parser 调用提取已注册子命令集合。"""
    if not cli_path.is_file():
        return set()
    text = cli_path.read_text(encoding="utf-8", errors="replace")
    # 匹配 add_parser("xxx") 或 add_parser('xxx')
    cmds = set(re.findall(r'add_parser\(\s*["\']([a-z][a-z-]*)["\']', text))
    return cmds


def check_ic1_obsolete_tech(skill_md: Path, cli_path: Path) -> list[IterationFinding]:
    """IC-1：扫描 SKILL.md 的过时技术栈关键词 + 幽灵命令引用。

    只报"命令引用/机制描述"形式，不报历史 changelog 提及（如 v3.1 说明里的"v3.0 新增 sync-tools"）。
    判定：行含 `ae-sdd <ghost>` 命令引用，或行描述机制（如"rules.yaml + 代码生成"）才算残留。
    """
    findings: list[IterationFinding] = []
    if not skill_md.is_file():
        return findings
    lines = skill_md.read_text(encoding="utf-8", errors="replace").splitlines()

    registered = _extract_cli_commands(cli_path)

    for i, line in enumerate(lines, 1):
        # 幽灵命令引用（ae-sdd <cmd> 形式）——这是真残留
        for ghost in _GHOST_COMMANDS:
            if re.search(rf"\bae-sdd\s+{re.escape(ghost)}\b", line):
                findings.append(IterationFinding(
                    check_id="IC-1", severity="warn",
                    item=f"幽灵命令 'ae-sdd {ghost}'",
                    location=f"{skill_md.name}:{i}",
                    detail=f"CLI 未注册此命令（已注册: {sorted(registered)[:8]}...）",
                ))
        # 过时技术栈关键词——只报"机制描述"（rules.yaml/sync-tools 作为机制名出现），
        # 不报历史 changelog 提及（含 🆕/v3.x/变更日志 等历史标记的行跳过）
        is_changelog = bool(re.search(r"🆕|v3\.\d|变更|changelog|历史|废弃|删除", line, re.IGNORECASE))
        if not is_changelog:
            for kw in _OBSOLETE_TECH_KEYWORDS:
                if kw in line:
                    findings.append(IterationFinding(
                        check_id="IC-1", severity="warn",
                        item=f"过时技术栈关键词 '{kw}'",
                        location=f"{skill_md.name}:{i}",
                        detail="v3.0 设计残留（rules.yaml/.mjs/sync-tools），当前是 Python CLI，应删除或改为正确描述",
                    ))
    return findings


# ─── IC-2 F-1 交叉验证覆盖面 ──────────────────────────────────────────────────
def check_ic2_gate_claim_coverage(stop_check_py: Path, gates_py: Path) -> list[IterationFinding]:
    """IC-2：统计 stop_check.py 的 GATE 交叉验证覆盖几个 gate；识别 v3.6 自报检测废弃。"""
    findings: list[IterationFinding] = []
    if not stop_check_py.is_file():
        return findings
    text = stop_check_py.read_text(encoding="utf-8", errors="replace")

    self_report_retired = (
        "废弃 _verify_gate_claims" in text
        or "废弃自报标记检测" in text
        or "流程合规性检测已全部转移到 UserPromptSubmit hook" in text
    )

    # 🆕 v3.5.10 Gap-012：识别两种覆盖模式
    #   模式 A（旧）：_G08_CLEAR_RE / _G\d+_CLEAR_RE 单 gate 正则
    #   模式 B（新）：_GATE_CLEAR_RE 通用正则 + _VERIFIABLE_GATE_IDS 集合
    covered: set[str] = set()
    # 模式 A
    covered.update(f"G-{g}" for g in re.findall(r"_G(\d+)_CLEAR_RE", text))
    # 模式 B：通用 _GATE_CLEAR_RE 命中 → 解析 _VERIFIABLE_GATE_IDS 集合
    has_generic_re = "_GATE_CLEAR_RE" in text
    if has_generic_re:
        ids_match = re.findall(r'"(G-\d+)"', text)
        # _VERIFIABLE_GATE_IDS 集合内的字面量
        verifiable_match = re.search(r"_VERIFIABLE_GATE_IDS\s*=\s*\{([^}]*)\}", text, re.DOTALL)
        if verifiable_match:
            ids_in_set = re.findall(r'"(G-\d+)"', verifiable_match.group(1))
            covered.update(ids_in_set)
        elif ids_match:
            covered.update(ids_match)

    # 统计 GATE_REGISTRY 总数
    total_gates = 0
    if gates_py.is_file():
        gates_text = gates_py.read_text(encoding="utf-8", errors="replace")
        total_gates = len(re.findall(r'"G-\d+"', gates_text))

    if self_report_retired and not covered:
        findings.append(IterationFinding(
            check_id="IC-2", severity="info",
            item="F-1 GATE 自报交叉验证已废弃（Stop hook 不再相信 ◆ GATE 自报）",
            location=f"{stop_check_py.name}",
            detail=(
                "v3.6 决策 1B：Stop hook 不再做 ◆ GATE 自报标记交叉验证，"
                "流程合规性转移到 UserPromptSubmit hook + flow_monitor + gates check。"
                "这是诚实降级，不再按旧 _G08_CLEAR_RE 覆盖面报 warn。"
            ),
        ))
    # 🆕 v3.5.10：覆盖 ≥ 5 个 gate 即视为"已扩展"，不再 warn
    elif len(covered) <= 1 and total_gates > 1:
        findings.append(IterationFinding(
            check_id="IC-2", severity="warn",
            item=f"F-1 交叉验证仅覆盖 {len(covered)} 个 gate（{sorted(covered)}）",
            location=f"{stop_check_py.name}",
            detail=(
                f"HS-12 声明'AI 谎报 ◆ GATE 声明'泛指，但 stop_check.py 只硬编码了 "
                f"{sorted(covered)} 的 CLEAR_RE 正则，其余 {total_gates - len(covered)} 个 gate 谎报不校验。"
                f"建议扩展 _G08_CLEAR_RE 为多 gate 循环，或在 HS-12 措辞中明确'当前覆盖 G-08'"
            ),
        ))
    elif len(covered) >= 5:
        findings.append(IterationFinding(
            check_id="IC-2", severity="info",
            item=f"F-1 交叉验证已扩展覆盖 {len(covered)} 个 gate（{sorted(covered)[:6]}...）",
            location=f"{stop_check_py.name}",
            detail=f"v3.5.10 Gap-012 修复：stop_check.py 已用 _GATE_CLEAR_RE 通用正则 + _VERIFIABLE_GATE_IDS 覆盖多 gate，覆盖面达标。",
        ))
    return findings


# ─── IC-3 已实现未接入扫描 ────────────────────────────────────────────────────
def check_ic3_unimported_modules(tools_dir: Path) -> list[IterationFinding]:
    """IC-3：扫 tools/lib/*.py 实现了但全树零 import 的模块 + git untracked。"""
    findings: list[IterationFinding] = []
    lib_dir = tools_dir / "lib"
    if not lib_dir.is_dir():
        return findings

    # 收集所有 tools/lib/*.py 模块名（不含 __init__）
    modules = [p.stem for p in lib_dir.glob("*.py") if p.stem != "__init__"]

    # 读取 tools/ 下所有 .py 文件 + tools/bin/ae-sdd（无 .py 后缀）的 import 语句
    imported: set[str] = set()
    py_files = list(tools_dir.rglob("*.py"))
    cli = tools_dir / "bin" / "ae-sdd"
    if cli.is_file():
        py_files.append(cli)
    for py in py_files:
        try:
            text = py.read_text(encoding="utf-8", errors="replace")
        except Exception:
            continue
        # 1. from lib import xxx, yyy, zzz as mod  →  解析整行提取所有模块名
        for m in re.findall(r"from\s+lib\s+import\s+([^\n]+)", text):
            for name in re.findall(r"\b([a-z_]\w*)\b", m):
                imported.add(name)
        # 2. from lib.xxx import yyy  →  提取 xxx
        for m in re.findall(r"from\s+lib\.(\w+)\s+import", text):
            imported.add(m)
        # 3. import lib.xxx  →  提取 xxx
        for m in re.findall(r"import\s+lib\.(\w+)", text):
            imported.add(m)
        # 4. 运行时动态 import：__import__("lib.xxx") 或 importlib.import_module("lib.xxx")
        for m in re.findall(r'import(?:_module)?\(["\']lib\.(\w+)["\']', text):
            imported.add(m)

    # 模块定义但未被任何文件 import → 候选
    # 🆕 v3.5.10 Gap-013：排除"AI Agent 调用面 API"模块——这些模块设计上就是
    # 给 LLM Agent 通过 SKILL 文档调用（resolve_path / save_doc 等），不期望被
    # CLI/gates 内部 import。零 import 不算死代码，避免 IC-3 误报。
    # 判定：模块 docstring 含 "AI Agent 调用" / "LLM 调用" / "Agent 调用面" 标记。
    _AGENT_API_MARKERS = ("AI Agent 调用", "LLM 调用", "Agent 调用面",
                          "给 LLM", "给 AI Agent", "Python SSOT",
                          "代码层实现", "代码兜底")
    for mod in modules:
        if mod not in imported:
            # 检查模块是否声明为"AI Agent 调用面 API"
            mod_file = lib_dir / f"{mod}.py"
            try:
                mod_text = mod_file.read_text(encoding="utf-8", errors="replace")
            except Exception:
                mod_text = ""
            is_agent_api = any(marker in mod_text[:2000] for marker in _AGENT_API_MARKERS)
            if is_agent_api:
                findings.append(IterationFinding(
                    check_id="IC-3", severity="info",
                    item=f"模块 '{mod}' 为 AI Agent 调用面 API（零 import 属设计预期，非死代码）",
                    location=f"tools/lib/{mod}.py",
                    detail=(
                        "本模块设计上由 LLM Agent 通过 SKILL 文档调用（如 resolve_path/save_doc），"
                        "不期望被 CLI/gates 内部 import。零 import 属预期，已从 warn 降级 info（v3.5.10 Gap-013）。"
                    ),
                ))
                continue
            findings.append(IterationFinding(
                check_id="IC-3", severity="warn",
                item=f"模块 '{mod}' 已实现但全树零 import（未接入运行时）",
                location=f"tools/lib/{mod}.py",
                detail=(
                    "实现存在但 CLI/gates 未真实调用，可能：① WIP 待接入；② 死代码。"
                    "需人工确认 design/skill 文档是否声明该能力——声明无实现=撒谎，已实现未接入=WIP未闭环"
                ),
            ))

    # git untracked 的 tools/lib/*.py
    try:
        result = subprocess.run(
            ["git", "status", "--short", "--", "tools/lib/"],
            cwd=tools_dir.parent, capture_output=True, text=True, timeout=10,
        )
        for line in result.stdout.splitlines():
            line = line.strip()
            if line.startswith("??") and line.endswith(".py") and "/lib/" in line:
                fname = line.split(maxsplit=1)[-1] if " " in line else line[3:]
                findings.append(IterationFinding(
                    check_id="IC-3", severity="warn",
                    item=f"未提交文件 '{fname}'（git untracked）",
                    location=fname,
                    detail="实现已写但未 git add + 未接入运行时，属 WIP 未闭环",
                ))
    except Exception:
        pass  # git 不可用则跳过

    return findings


# ─── IC-4 HS 物理实现粗筛 ─────────────────────────────────────────────────────
# HARNESS.md HS-N → 三个 hook 文件的关键词映射（声明在哪个文件实现）
# 注：这是粗筛（关键词存在性），"声明物理拦截但 stub/缩水"需人工确认（iteration-check SOP 步骤2）
_HARNESS_HS_IMPL_HINT: dict[str, tuple[str, str]] = {
    # HS-N: (hook_file_name, 应出现的关键词)
    "HS-1": ("gate_intercept.py", "src/main"),
    "HS-2": ("gate_intercept.py", "state write"),
    "HS-7": ("gate_intercept.py", "prd-complete"),
    "HS-8": ("stop_check.py", "awaiting_compact"),
    "HS-9": ("gate_intercept.py", "entry token"),
    "HS-10": ("gate_intercept.py", "resolve_path"),
    "HS-11": ("gate_intercept.py", "task-reviewed"),
    "HS-12": ("stop_check.py", "GATE"),
}


def check_ic4_hs_physical_impl(harness_md: Path, tools_dir: Path) -> list[IterationFinding]:
    """IC-4：HARNESS.md 声明的 HS-N vs 三 hook 文件实际实现粗筛。"""
    findings: list[IterationFinding] = []
    if not harness_md.is_file():
        return findings
    harness_text = harness_md.read_text(encoding="utf-8", errors="replace")

    # 提取 HARNESS.md 声明的所有 HS-N
    declared_hs = set(re.findall(r"\bHS-(\d+)\b", harness_text))

    lib_dir = tools_dir / "lib"
    hook_texts: dict[str, str] = {}
    for name in ("gate_intercept.py", "stop_check.py", "prompt_inject.py"):
        p = lib_dir / name
        if p.is_file():
            hook_texts[name] = p.read_text(encoding="utf-8", errors="replace")

    for hs_num in sorted(declared_hs, key=int):
        hs_id = f"HS-{hs_num}"
        hint = _HARNESS_HS_IMPL_HINT.get(hs_id)
        if hint is None:
            # HS-3/4/5/6 等无映射的：检查是否在 HARNESS.md 自认"声明但无物理实现"
            # （HS-3/5 已自认降级，HS-4/6 v3.5.4 降级）
            hs_line_match = re.search(rf"{hs_id}[^\n]*", harness_text)
            hs_line = hs_line_match.group(0) if hs_line_match else ""
            if "声明但无物理实现" in hs_line or "靠" in hs_line and "自律" in hs_line:
                findings.append(IterationFinding(
                    check_id="IC-4", severity="info",
                    item=f"{hs_id} 已诚实自认降级（靠自律/兜底）",
                    location=f"HARNESS.md",
                    detail=hs_line[:80],
                ))
            else:
                findings.append(IterationFinding(
                    check_id="IC-4", severity="warn",
                    item=f"{hs_id} 声明 HARD STOP 但无物理实现映射，需人工确认是否降级声明",
                    location=f"HARNESS.md",
                    detail="建议补'声明但无物理实现，靠 XX 兜底'自认降级措辞（与 HS-3/5 对齐）",
                ))
            continue

        hook_file, keyword = hint
        hook_text = hook_texts.get(hook_file, "")
        if keyword not in hook_text:
            findings.append(IterationFinding(
                check_id="IC-4", severity="warn",
                item=f"{hs_id} 声明物理拦截但 {hook_file} 零提及 '{keyword}'",
                location=f"tools/lib/{hook_file}",
                detail="声明物理拦截实为零实现（文档撒谎），需补实现或降级声明",
            ))
        else:
            findings.append(IterationFinding(
                check_id="IC-4", severity="info",
                item=f"{hs_id} 物理实现关键词存在（粗筛通过，语义缩水需人工确认）",
                location=f"tools/lib/{hook_file}",
                detail=f"关键词 '{keyword}' 存在；但'声明物理拦截 vs 实际逻辑真接'需人工核对（SOP 步骤2）",
            ))
    return findings


# ─── 主入口 ───────────────────────────────────────────────────────────────────
def run_all(repo_root: Path) -> IterationReport:
    """跑 IC-1 ~ IC-4 全部检查，返回报告（不阻断）。"""
    report = IterationReport()
    source = repo_root / "source"
    tools = repo_root / "tools"

    skill_md = source / "SKILL.md"
    cli_path = tools / "bin" / "ae-sdd"
    harness_md = source / "HARNESS.md"
    stop_check_py = tools / "lib" / "stop_check.py"
    gates_py = tools / "lib" / "gates.py"

    # IC-1
    report.findings.extend(check_ic1_obsolete_tech(skill_md, cli_path))
    report.checks_run.append("IC-1")

    # IC-2
    report.findings.extend(check_ic2_gate_claim_coverage(stop_check_py, gates_py))
    report.checks_run.append("IC-2")

    # IC-3
    report.findings.extend(check_ic3_unimported_modules(tools))
    report.checks_run.append("IC-3")

    # IC-4
    report.findings.extend(check_ic4_hs_physical_impl(harness_md, tools))
    report.checks_run.append("IC-4")

    return report
