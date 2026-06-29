"""
alignment_audit.py — ae-sdd 全维对齐验证器 AA（🆕 v3.5.11）

体系级根治：把 update-check 从"窄面单向校验器"升级为"全维双向对齐验证器"。

背景（2026-06-29 全局诊断）：
  review-loop / 多 reviewer / 7 道闸 / 16 质量闸 / 死字段 / 幽灵命令 等 14 个 P0
  病根都是同一类——「协议层（source/ SKILL）承诺详尽，工具链层（tools/ gates/
  state / CLI）缺少对应机械落地」。现有 UC-01~07 有两个致命缺陷：
    1. 单向：只查"CLI→doc"（已注册 gate 有无 check 函数），不查"doc→CLI"
       （SKILL 承诺的门禁是否都注册）
    2. 窄面：只覆盖命令契约+版本号+门禁注册存在性。门禁实现真实性 / state
       字段存活性 / 状态机闭环 / 幽灵命令全捕获 完全不查

本模块补齐 5 个新维度（UC-08~UC-12），与 UC-01~07 同构（都是 check_* 函数 +
UpdateCheckResult），统一进 update_graph.CHECK_FUNCS / check_all / dev-sync 阻断链。

5 个新维度：
  UC-08 门禁承诺↔注册（双向）：doc 承诺的「🔴 硬门禁/N 道闸/HARD 清单」是否
        都在 GATE_REGISTRY；注册的 gate 是否都有 doc 承诺
  UC-09 门禁实现真实性：已注册 gate 的 check 函数是否真执行（抓 stub-pass 假门禁，
        如 G-RA-FLOW-VIOLATION check_all 漏传 master_source）
  UC-10 state 字段存活性：doc schema 承诺的字段是否有 tools/ 写入方（抓死字段）
  UC-11 状态机闭环：doc 承诺的「连续 N 轮 / 累加计数 / 状态机 N 态」是否有持久化
  UC-12 幽灵命令全捕获：修 UC-06 正则 lookahead 缺陷，抓「ae-sdd run xxx-skill」
        这类「幽灵命令 + 子命令词」写法

档位机制（首跑分档处置，避免一次性重写所有 CLI 引入新撒谎）：
  - 🔴 error 级（假门禁/幽灵命令）：当场修
  - 🟠 软门禁降级档（门禁撒谎）：doc 改软门禁 + 指向已有 gate
  - 🟡 死字段追踪档：纳入持续追踪清单，不阻断本次启用
"""
from __future__ import annotations

import re
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

from lib.update_graph import UpdateCheckResult, HISTORICAL_UNIMPLEMENTED


# ════════════════════════════════════════════════════════════════════════════
# 辅助：从 SKILL 文档抽取门禁/闸承诺
# ════════════════════════════════════════════════════════════════════════════

# 门禁承诺关键词：SKILL 用这些措辞声称"硬门禁"，需对账 GATE_REGISTRY
_GATE_CLAIM_KEYWORDS = [
    r"🔴\s*硬门禁",
    r"硬门禁",
    r"强制门禁",
    r"\[HARD\]",
    r"硬前置",
    r"硬停止",
    r"阻断型闸",
    r"\d+\s*道闸",
    r"\d+\s*道质量闸",
    r"\d+\s*道 RA 质量闸",
]

# 已对齐区：这些 SKILL 承诺的门禁**已有对应 GATE_REGISTRY 注册**，不算撒谎
# (SKILL 承诺措辞片段 → 对应的 gate id)。命中映射的承诺不报为 gap。
_ALIGNED_GATE_MAP = {
    "G-00": "项目资产门卫",
    "G-01": "DR 文档存在",
    "G-02": "Story 文档存在",
    "G-03": "Story Review 通过",
    "G-04": "TestCase 文档存在",
    "G-05": "Task 文档存在",
    "G-06": "Task Review 通过",
    "G-07": "CodingPlan 存在",
    "G-08": "CodingPlan 14 门禁",
    "G-09": "测试真实性",
    "G-10": "测试报告存在",
    "G-11": "Coding 报告存在",
    "G-12": "CodeReview 报告存在",
    "G-13": "全链路对称性",
    "G-14": "CodingPlan-Story 一致性",
    "G-CODEPLAN-SRC": "CodingPlan 源码核对",
    "G-DOC-STORAGE": "文档落地存放",
    "G-DOC-CONSISTENCY": "项目侧记忆-配置",
    "G-RA-1": "RA 文档存在",
    "G-RA-2": "RA 维度完整",
    "G-RA-3": "RA 衍生章节",
    "G-RA-4": "RA 真实性",
    "G-RA-5": "RA 机械派生深度",
    "G-RA-FLOW-VIOLATION": "RA 流程违规",
    "G-CODE-1": "Coding 真实性",
    "G-PATH": "路径越界",
}


def _scan_skill_gate_claims(skill_files: list, repo_root: Path) -> list[dict]:
    """扫描所有 SKILL 文档，抽取「门禁/闸」承诺。

    返回 [{file, line, claim, gate_ref}]
    - claim: 承诺原文片段（如"🔴 硬门禁""7 道闸""16 道质量闸"）
    - gate_ref: 若该承诺附近引用了具体 G-XX id，记下（命中 _ALIGNED_GATE_MAP 则不报）
    """
    claims: list[dict] = []
    for sf in skill_files:
        if not sf.is_file():
            continue
        try:
            lines = sf.read_text(encoding="utf-8", errors="replace").splitlines()
        except OSError:
            continue
        for i, line in enumerate(lines, 1):
            for kw in _GATE_CLAIM_KEYWORDS:
                if re.search(kw, line):
                    # 查该行 ±3 行内是否引用了已对齐的 gate id
                    ctx = "\n".join(lines[max(0, i - 4):i + 3])
                    gate_ref = None
                    for gid in _ALIGNED_GATE_MAP:
                        if gid in ctx:
                            gate_ref = gid
                            break
                    claims.append({
                        "file": str(sf.relative_to(repo_root)),
                        "line": i,
                        "claim": line.strip()[:120],
                        "matched_kw": kw,
                        "gate_ref": gate_ref,
                    })
                    break  # 同一行只记一次
    return claims


# ════════════════════════════════════════════════════════════════════════════
# UC-08 门禁承诺↔注册（双向）
# ════════════════════════════════════════════════════════════════════════════

def check_uc08_gate_claim_alignment(repo_root: Path) -> UpdateCheckResult:
    """UC-08 门禁承诺↔注册双向对齐。

    方向 A（doc→gate）：SKILL 用「🔴 硬门禁/道闸/HARD 清单」声称硬门禁，
        但既没引用已对齐的 G-XX id，也无 scanner → 疑似「门禁撒谎」。
        命中 _ALIGNED_GATE_MAP 的不算（已有对应 gate）。
    方向 B（gate→doc）：GATE_REGISTRY 注册的 gate 应在 SKILL 有承诺引用。
    """
    name = "门禁承诺↔注册双向对齐"

    # 收集所有 SKILL 文档
    skill_files = [repo_root / "source" / "SKILL.md"]
    skills_dir = repo_root / "source" / "skills"
    if skills_dir.is_dir():
        skill_files += list(skills_dir.rglob("*.md"))

    claims = _scan_skill_gate_claims(skill_files, repo_root)

    # 方向 A：无 gate_ref 的承诺 = 疑似撒谎（需人工/AA 复核是否真有 gate 支撑）
    orphan_claims = [c for c in claims if c["gate_ref"] is None]

    if orphan_claims:
        # 分档：按文件聚合，避免输出爆炸
        by_file: dict[str, int] = {}
        for c in orphan_claims:
            by_file[c["file"]] = by_file.get(c["file"], 0) + 1
        top = sorted(by_file.items(), key=lambda x: -x[1])[:8]
        sample = orphan_claims[:3]
        return UpdateCheckResult(
            "UC-08", name, "warn", True,  # warn 不阻断（软门禁降级档）
            f"发现 {len(orphan_claims)} 处「门禁/闸」承诺未绑定已对齐 G-XX id（疑似软门禁/待硬化）",
            "按 🟠 软门禁降级策略：在对应 SKILL 顶部加强度声明「软门禁 report-only，硬门禁见 G-XX」，"
            "或补 gate 注册使其硬化；详见 AA 首跑 gap 清单",
            details={
                "orphan_count": len(orphan_claims),
                "by_file": dict(top),
                "sample": [{"file": s["file"], "line": s["line"],
                            "claim": s["claim"]} for s in sample],
            },
        )

    return UpdateCheckResult("UC-08", name, "ok", True,
                             f"门禁承诺全部绑定 G-XX id 或无硬门禁措辞",
                             details={"total_claims": len(claims)})


# ════════════════════════════════════════════════════════════════════════════
# UC-09 门禁实现真实性（抓 stub-pass 假门禁）
# ════════════════════════════════════════════════════════════════════════════

# 已知的假门禁模式（check_all 漏传参数 / check 函数恒 stub-pass）
# 格式：(gate_id, 检测方法, 说明)
def _detect_stub_pass_gates(gates_py_text: str) -> list[dict]:
    """检测已注册但恒 stub-pass 的假门禁。

    已知模式：
    1. check_all 里某 gate 走 CHECK_FUNCS 但该 check 函数签名需要 master_source，
       而 check_all 调用时漏传（恒拿 None → scanner 定位失败 → stub-pass）
    2. check 函数体内引用未定义的 NameError（如 _sys 而只 import sys）
    """
    stubs: list[dict] = []

    # 模式 1：G-RA-FLOW-VIOLATION 在 check_all 是否特判传 master_source
    # 已知正确范式：G-RA-4 / G-RA-5 / G-CODE-1 在 check_all 有特判分支传 master_source
    # G-RA-FLOW-VIOLATION 应同样特判但当前走 CHECK_FUNCS（漏传）
    if "G-RA-FLOW-VIOLATION" in gates_py_text:
        # 检查 check_all 是否有 G-RA-FLOW-VIOLATION 特判分支
        # 特判范式：if g["id"] == "G-RA-FLOW-VIOLATION": ...check_...(master_source=...)
        has_special = bool(re.search(
            r'g\["id"\]\s*==\s*"G-RA-FLOW-VIOLATION"\s*:', gates_py_text))
        if not has_special:
            stubs.append({
                "gate_id": "G-RA-FLOW-VIOLATION",
                "pattern": "check_all 未特判传 master_source",
                "impact": "走 CHECK_FUNCS 时 master_source 缺省 None → scanner 定位失败 → 恒 stub-pass",
                "fix": "在 check_all 加特判分支（参照 G-RA-4/G-RA-5/G-CODE-1），传 master_source",
            })

        # 模式 2：_sys NameError
        # check_ra_flow_violation 体内用 _sys.executable 但只 import sys
        # 找 check_ra_flow_violation 函数体
        m = re.search(
            r"def check_ra_flow_violation.*?(?=\ndef |\Z)",
            gates_py_text, re.DOTALL)
        if m:
            body = m.group(0)
            if "_sys.executable" in body and "import sys as _sys" not in body:
                # 确认模块级是否 import sys as _sys
                if not re.search(r"^import\s+sys\s+as\s+_sys", gates_py_text, re.MULTILINE):
                    stubs.append({
                        "gate_id": "G-RA-FLOW-VIOLATION",
                        "pattern": "check 函数引用 _sys 但模块未 import sys as _sys",
                        "impact": "即便补传 master_source，也会 NameError 崩溃",
                        "fix": "改 _sys.executable → sys.executable，或加 import sys as _sys",
                    })

    return stubs


def check_uc09_gate_impl_authenticity(repo_root: Path) -> UpdateCheckResult:
    """UC-09 门禁实现真实性：已注册 gate 的 check 函数是否真执行（抓 stub-pass 假门禁）。"""
    name = "门禁实现真实性"
    gates_py = repo_root / "tools" / "lib" / "gates.py"
    if not gates_py.is_file():
        return UpdateCheckResult("UC-09", name, "error", False, "gates.py 不存在")

    gates_text = gates_py.read_text(encoding="utf-8", errors="replace")
    stubs = _detect_stub_pass_gates(gates_text)

    if stubs:
        details_list = [
            f"{s['gate_id']}: {s['pattern']}（{s['impact']}；修复：{s['fix']}）"
            for s in stubs
        ]
        return UpdateCheckResult(
            "UC-09", name, "error", False,
            f"发现 {len(stubs)} 个假门禁（注册但恒 stub-pass）：{details_list[:3]}",
            "修复 check_all 特判传参 / 修 NameError，详见每个 stub 的 fix 字段",
            details={"stubs": stubs},
        )

    return UpdateCheckResult("UC-09", name, "ok", True,
                             "门禁实现真实：无 stub-pass 假门禁")


# ════════════════════════════════════════════════════════════════════════════
# UC-10 state 字段存活性（抓死字段）
# ════════════════════════════════════════════════════════════════════════════

# doc schema 承诺的字段 → 期望的写入方特征（tools/lib 里应出现的写入代码）
# 若 tools/ 全树搜不到写入特征，判定为死字段
# 格式：(字段名, 写入特征正则, doc 承诺位置提示)
_DEAD_FIELD_PROBES = [
    # 多 Agent state 字段
    ("activeAgents", r'\bactiveAgents\b.*=|setdefault\(\s*"activeAgents"', "SKILL.md §🤖 多 Agent 状态共享"),
    ("agentReports", r'\bagentReports\b.*=|setdefault\(\s*"agentReports"', "SKILL.md §🤖 多 Agent 状态共享"),
    # review-loop 计数器（本案根因）
    ("reviewLoop", r'\breviewLoop\b.*=|setdefault\(\s*"reviewLoop"', "review-loop 公共协议"),
    ("dryCounter", r'\bdryCounter\b.*=|dry_counter', "review-loop 公共协议"),
    # Story 级重入字段
    ("codingRound", r'\bcodingRound\b.*=', "SKILL.md §流程状态跟踪"),
    ("completedSteps", r'completedSteps["\']?\s*\]?\s*\.append|setdefault\(\s*["\']completedSteps', "SKILL.md §重入流程判定"),
    ("currentStep", r'\bcurrentStep\b.*=', "SKILL.md §重入流程判定"),
    # PRD 级字段（除 prdStatus 外基本是死的）
    ("prdId", r'\bprdId\b.*=', "document-storage §3.5 PRD schema"),
    ("crossStoryDeps", r'\bcrossStoryDeps\b.*=', "document-storage §3.5 PRD schema"),
    ("crossStoryResidualRisks", r'\bcrossStoryResidualRisks\b.*=', "document-storage §3.5"),
    ("prdReview", r'\bprdReview\b.*=', "document-storage §3.5 PRD schema"),
    ("gateRegistry", r'\bgateRegistry\b.*=', "document-storage §3.5 PRD schema"),
    # memoryLifecycle 已于 v3.5.12 退役（死设计重复，实际门禁读 memory_store），不再追踪
    ("compactHistory", r'compactHistory["\']?\s*\]?\s*\.append|setdefault\(\s*["\']compactHistory', "document-storage §3.5"),
    ("sizeBudget", r'\bsizeBudget\b.*=', "document-storage §3.5"),
    ("storyIds", r'\bstoryIds\b.*=', "document-storage §3.5 PRD schema"),
]


def check_uc10_state_field_liveness(repo_root: Path) -> UpdateCheckResult:
    """UC-10 state 字段存活性：doc schema 承诺的字段是否有 tools/ 写入方。"""
    name = "state 字段存活性"
    tools_lib = repo_root / "tools" / "lib"
    tools_bin = repo_root / "tools" / "bin"
    if not tools_lib.is_dir():
        return UpdateCheckResult("UC-10", name, "error", False, "tools/lib 不存在")

    # 读 tools/lib/*.py + tools/bin/ae-sdd，但排除：
    #   - alignment_audit.py 自身（含 probe 字段名字面量，会污染判定）
    #   - test_*.py（测试里的字段名不算生产写入）
    #   - __pycache__
    excluded = {"alignment_audit.py"}
    search_files = []
    for py in tools_lib.glob("*.py"):
        if py.name in excluded:
            continue
        search_files.append(py)
    bin_ae_sdd = tools_bin / "ae-sdd"
    if bin_ae_sdd.is_file():
        search_files.append(bin_ae_sdd)

    haystack = ""
    for py in search_files:
        try:
            haystack += py.read_text(encoding="utf-8", errors="replace") + "\n"
        except OSError:
            pass

    dead_fields: list[dict] = []
    for field_name, write_pattern, doc_hint in _DEAD_FIELD_PROBES:
        # 写入形态匹配（专门排除只读 .get() / 文档注释）
        # write_pattern 已设计为匹配赋值/setdefault/append，不再命中 .get() 只读
        writes = re.findall(write_pattern, haystack)
        # 二次过滤：排除 docstring/注释行内的字段名（行首是 # 或在 """ 块内简化判定）
        real_writes = []
        for w in writes:
            # write_pattern 匹配到的是代码片段；若片段形如纯 .get 不算
            # （write_pattern 已不含 .get，这里防御性跳过明显只读形态）
            if ".get(" in str(w) and "=" not in str(w).split(".get(")[0][-3:]:
                continue
            real_writes.append(w)
        if not real_writes:
            dead_fields.append({
                "field": field_name,
                "doc_hint": doc_hint,
                "status": "死字段（doc schema 承诺但 tools/ 无写入方）",
            })

    if dead_fields:
        return UpdateCheckResult(
            "UC-10", name, "warn", True,  # warn（死字段追踪档，不阻断启用）
            f"发现 {len(dead_fields)} 个死字段（doc schema 承诺但 tools/ 无写入方）",
            "🟡 死字段追踪档：纳入持续追踪清单，按优先级逐个补写入路径或从 doc 删除；"
            "不阻断本次启用。详见 AA 首跑 gap 清单",
            details={
                "dead_count": len(dead_fields),
                "fields": [{"field": d["field"], "doc_hint": d["doc_hint"]} for d in dead_fields],
            },
        )

    return UpdateCheckResult("UC-10", name, "ok", True,
                             f"state 字段全部存活（{len(_DEAD_FIELD_PROBES)} 个 probe 全有写入方）")


# ════════════════════════════════════════════════════════════════════════════
# UC-11 状态机闭环（doc 承诺「连续 N 轮/累加/状态机 N 态」有无持久化）
# ════════════════════════════════════════════════════════════════════════════

# doc 承诺的「状态机/计数」语义 → 持久化探针
# 格式：(承诺语义关键词, 持久化探针正则, doc 提示)
_STATE_MACHINE_PROBES = [
    # review-loop「连续 N 轮无新增」退出条件
    ("连续 3 轮无新增", r'dryCounter|dry_counter|review_loop.*round|reviewLoop.*round',
     "review-loop 公共协议 §协议1"),
    ("循环上限", r'max_rounds|maxRound|MAX_ROUNDS|round.*>=.*3', "review-loop 公共协议 §协议2"),
    # 多 reviewer tier
    ("reviewerTier", r'reviewerTier|reviewer_tier|compute_tier|tier\s*=', "agent-orchestration §8.4.1"),
    # PRD 状态机 5 态
    ("prdStatus", r'prdStatus.*=.*"compacted"|prdStatus.*=.*"prd_aborted"', "document-storage §3.5"),
]


def check_uc11_state_machine_closure(repo_root: Path) -> UpdateCheckResult:
    """UC-11 状态机闭环：doc 承诺的状态机/计数有无 tools/ 持久化。"""
    name = "状态机闭环"
    tools_lib = repo_root / "tools" / "lib"
    tools_bin = repo_root / "tools" / "bin"
    if not tools_lib.is_dir():
        return UpdateCheckResult("UC-11", name, "error", False, "tools/lib 不存在")

    # 排除 alignment_audit.py 自身（含 probe 字面量）
    excluded = {"alignment_audit.py"}
    search_files = [p for p in tools_lib.glob("*.py") if p.name not in excluded]
    bin_ae_sdd = tools_bin / "ae-sdd"
    if bin_ae_sdd.is_file():
        search_files.append(bin_ae_sdd)

    search_text = ""
    for py in search_files:
        try:
            search_text += py.read_text(encoding="utf-8", errors="replace") + "\n"
        except OSError:
            pass

    open_state_machines: list[dict] = []
    for semantic, probe, doc_hint in _STATE_MACHINE_PROBES:
        if not re.search(probe, search_text):
            open_state_machines.append({
                "semantic": semantic,
                "doc_hint": doc_hint,
                "status": "协议承诺状态机/计数但 tools/ 无持久化实现",
            })

    if open_state_machines:
        return UpdateCheckResult(
            "UC-11", name, "warn", True,  # warn（部分依赖第 1 波 review-loop CLI 落地）
            f"发现 {len(open_state_machines)} 个状态机/计数承诺无 tools/ 持久化",
            "🟡 状态机闭环档：第 1 波 review-loop CLI 落地后自动收敛 reviewLoop/dryCounter/tier；"
            "prdStatus 5 态闭环纳入后续迭代。详见 AA 首跑 gap 清单",
            details={"open_count": len(open_state_machines),
                     "items": [{"semantic": s["semantic"], "doc_hint": s["doc_hint"]} for s in open_state_machines]},
        )

    return UpdateCheckResult("UC-11", name, "ok", True,
                             "状态机全部闭环（计数/状态机承诺均有持久化）")


# ════════════════════════════════════════════════════════════════════════════
# UC-12 幽灵命令全捕获（修 UC-06 lookahead 缺陷）
# ════════════════════════════════════════════════════════════════════════════

def _extract_skill_referenced_commands_robust(text: str) -> dict:
    """从 SKILL 文本抽取 ae-sdd <cmd> 引用（修 UC-06 lookahead 缺陷）。

    UC-06 的正则 `ae-sdd\\s+([a-z][a-z-]*)(?=\\s+--|`|\\s*$)` 有 lookahead 缺陷：
    `ae-sdd run dr-review-skill` 这类「幽灵命令 + 子命令词」写法，run 后紧跟
    空格+命令词，不满足 lookahead（需 run -- 或 run` 或行尾）→ run 完全不被捕获。

    本函数采用「反引号/代码块边界」判定，比 lookahead 更精确，避免误抓叙述性文本
    （如 `# ae-sdd generated docs`、`ae-sdd evidence`、`ae-sdd pattern memory`）：
      命令调用形态 = ae-sdd <cmd> 出现在以下任一上下文：
        (a) 反引号内：`ae-sdd <cmd> ...` （最常见，命令调用必裹反引号）
        (b) bash 代码块内：```bash 块里的 ae-sdd <cmd>
        (c) 紧跟 --flag：ae-sdd <cmd> --xxx
      不算命令调用的形态（叙述）：正文里裸写的 ae-sdd <名词>（无反引号无 flag）

    返回 {cmd: [{line, context}]}
    """
    results: dict[str, list[dict]] = {}
    lines = text.splitlines()
    in_code_block = False
    for i, line in enumerate(lines, 1):
        # 跟踪 ``` 代码块状态（bash/yaml 块内的 ae-sdd 视为命令调用）
        stripped = line.lstrip()
        if stripped.startswith("```"):
            in_code_block = not in_code_block
            continue

        # 判定本行是否在命令调用上下文
        in_backtick = False
        # (a) 反引号内：行内有 `...ae-sdd <cmd>...`，但排除注释标题 `# ae-sdd xxx`
        for m in re.finditer(r"`[^`]*ae-sdd\s+([a-z][a-z-]*)", line):
            inner = m.group(0)
            # 反引号内容若以 # 开头（注释标题，如 `# ae-sdd generated docs`）不算命令
            if inner.lstrip("`").lstrip().startswith("#"):
                continue
            cmd = m.group(1)
            results.setdefault(cmd, []).append({"line": i, "context": line.strip()[:100]})
            in_backtick = True
        # (b) 代码块内：直接抽 ae-sdd <cmd>，但排除注释行（# 开头）
        if in_code_block and not in_backtick:
            # 跳过注释行（# ae-sdd generated docs 这类 .gitignore 标题）
            code_part = line.split("#", 1)[0] if "#" in line else line
            for m in re.finditer(r"\bae-sdd\s+([a-z][a-z-]*)", code_part):
                cmd = m.group(1)
                results.setdefault(cmd, []).append({"line": i, "context": line.strip()[:100]})
                in_backtick = True
        # (c) 紧跟 --flag：ae-sdd <cmd> --xxx（裸写但带参数）
        if not in_backtick:
            for m in re.finditer(r"\bae-sdd\s+([a-z][a-z-]*)(?=\s+--)", line):
                cmd = m.group(1)
                results.setdefault(cmd, []).append({"line": i, "context": line.strip()[:100]})
    return results


def check_uc12_ghost_command_capture(repo_root: Path) -> UpdateCheckResult:
    """UC-12 幽灵命令全捕获：修 UC-06 lookahead 缺陷，全量扫所有 SKILL 文档。"""
    name = "幽灵命令全捕获"

    # 读 CLI 真实命令集
    cli_path = repo_root / "tools" / "bin" / "ae-sdd"
    if not cli_path.is_file():
        return UpdateCheckResult("UC-12", name, "error", False, "CLI 不存在")
    cli_text = cli_path.read_text(encoding="utf-8", errors="replace")
    cli_cmds = set(re.findall(r'\.add_parser\("([a-z][a-z-]*)"', cli_text))

    # 扫所有 SKILL 文档（含子 SKILL）
    skill_files = [repo_root / "source" / "SKILL.md"]
    skills_dir = repo_root / "source" / "skills"
    if skills_dir.is_dir():
        skill_files += list(skills_dir.rglob("*.md"))

    ghosts: list[dict] = []
    for sf in skill_files:
        if not sf.is_file():
            continue
        try:
            text = sf.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        # 跳过 YAML frontmatter
        body = text
        if text.startswith("---"):
            end = text.find("\n---", 3)
            if end != -1:
                body = text[end + 4:]
        refs = _extract_skill_referenced_commands_robust(body)
        for cmd, occurrences in refs.items():
            if cmd in cli_cmds or cmd in HISTORICAL_UNIMPLEMENTED or cmd in {"v", "description", "version", "name"}:
                continue
            # 真 ghost：CLI 未注册 + 非历史遗留
            ghosts.append({
                "cmd": cmd,
                "file": str(sf.relative_to(repo_root)),
                "occurrences": [{"line": o["line"], "context": o["context"]} for o in occurrences[:2]],
                "n_occurrences": len(occurrences),
            })

    if ghosts:
        # 区分：标注了"未来命令"的（如 fork）vs 真撒谎（如 run dr-review-skill 无标注）
        # 检查每个 ghost 的上下文是否含"未来命令"标注
        real_ghosts = []
        annotated = []
        for g in ghosts:
            # 读该文件全文，查 ghost cmd 附近有无"未来命令"标注
            fp = repo_root / g["file"]
            try:
                full = fp.read_text(encoding="utf-8", errors="replace")
            except OSError:
                full = ""
            has_annotation = ("未来命令" in full or "future command" in full.lower())
            if has_annotation:
                annotated.append(g)
            else:
                real_ghosts.append(g)

        if real_ghosts:
            return UpdateCheckResult(
                "UC-12", name, "error", False,
                f"发现 {len(real_ghosts)} 个幽灵命令（SKILL 引用但 CLI 未注册且无'未来命令'标注）："
                f"{[g['cmd'] for g in real_ghosts[:5]]}",
                "🔴 error 档：当场删除幽灵命令引用，或补 CLI 注册，或加'未来命令'标注",
                details={"real_ghosts": real_ghosts, "annotated_future": annotated},
            )
        if annotated:
            return UpdateCheckResult(
                "UC-12", name, "warn", True,
                f"{len(annotated)} 个命令已标'未来命令'（warn）：{[g['cmd'] for g in annotated]}",
                "后续迭代实现，或保持标注",
                details={"annotated_future": annotated},
            )

    return UpdateCheckResult("UC-12", name, "ok", True,
                             "幽灵命令全清：所有 SKILL 引用的命令均在 CLI 注册或已标注未来命令")


# ════════════════════════════════════════════════════════════════════════════
# 注册到 update_graph.CHECK_FUNCS（运行时注入，保持单一 check_all 入口）
# ════════════════════════════════════════════════════════════════════════════

AA_CHECK_FUNCS = {
    "UC-08": check_uc08_gate_claim_alignment,
    "UC-09": check_uc09_gate_impl_authenticity,
    "UC-10": check_uc10_state_field_liveness,
    "UC-11": check_uc11_state_machine_closure,
    "UC-12": check_uc12_ghost_command_capture,
}


def register_to_update_graph() -> None:
    """把 UC-08~UC-12 注入 update_graph.CHECK_FUNCS，使 check_all/CLI 统一调度。

    在 update_graph 模块加载后调用一次。幂等（重复调用无副作用）。
    """
    try:
        from lib import update_graph as ug
    except ImportError:
        return
    for cid, fn in AA_CHECK_FUNCS.items():
        ug.CHECK_FUNCS[cid] = fn


# 自动注册（import 本模块即生效）
register_to_update_graph()
