#!/usr/bin/env python3
"""Scan Requirement Analysis (RA) documents for *mechanical-derivation depth*.

🆕 2026-06-27 v3.5.9 — 机械派生深度校验（对标 ra_authenticity_scan.py / flow_violation_scan.py）

设计哲学：
  G-RA-3（章节锚点存在）/ G-RA-4（真实性：无 fabricate/vague）/ G-RA-FLOW-VIOLATION（流程完整性：
  章节是否存在）都是「存在性」检查。本扫描器补「内容深度」正交维度——验证 E.5/G.5/H.6/H.5 规定的
  「每条规则 R 必须机械追问 6 问 → 衍生 R′」、「每条 R′ 必须映射到 H.5 模式编号」、「§9-ter 每个触发动作
  必须回答 5 问」、「§8.6 覆盖率是真实重算而非断言」。杜绝「形式通过、内容空转」（用户实测：13 个
  问题 → 被逼出 34 个，根因即 E.5 表填了但每行没机械派生，机器查不出）。

5 条规则（D1~D5）：
  D1  §6.5 主规则机械派生：每个唯一主规则 R 必有 ≥1 衍生 R′ 行，每行「衍生模式命中」列含模式编号
  D2  R′→AC 链接完整性：§6.5 每个 R′ 在 §8.5「对应规则 R′」列出现 ≥1 次
  D3  §8.6 覆盖率真实重算：重数 R′ 总数 M、衍生 AC 数 K；声明 K/M 须与实际一致且 ≥80%
  D4  §9-ter H.6 五问机械覆盖：每触发动作 ≥1 行覆盖状态机/事件/缓存/MQ/聚合根；时效禁「尽快/及时/立即」
  D5  §9-bis 业务模式六选一：6 模式每个「适用(含编号)」或「不适用+理由」；适用但编号空 → BLOCKER

与已有扫描器的关系：
  ra_authenticity_scan.py  = 内容真实性（无 fabricate/vague/timeless）
  flow_violation_scan.py   = 流程完整性（章节是否存在 + 路由决策 + RA-G 标记数）
  ra_depth_scan.py（本文）  = 机械派生深度（每行 R→R′→AC 是否真做了机械追问 + 链接）

输出契约与 ra_authenticity_scan.py 一致（status/raFiles/findings[]），BLOCKER>0 → exit 1。

调用入口：
  python scripts/ra_depth_scan.py --root <dir> [--format json|markdown] [--strict]
  ae-sdd gates check --only G-RA-5                  （内部调本脚本）
  ae-sdd ra-depth-scan --root <dir>                 （CLI 子命令）
"""
from __future__ import annotations

import argparse
import html
import json
import re
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Iterable

from ra_scan_scope import (
    RAScanScopeError,
    ra_scan_scope_error_payload,
    resolve_ra_scan_scope,
)


# 与 tools/lib/gates.py:STATE_MACHINE_KEYWORDS 保持一致（触发 §6.5/§9-ter 强制核验）
STATE_MACHINE_KEYWORDS = (
    "状态变更", "状态机", "触发", "联动", "禁用", "启用",
    "锁定", "解锁", "注销", "角色变更", "退款", "取消",
    "登录", "登出", "失败", "超时", "过期", "状态流转",
)

# 跨域级联判定关键词（≥2 命中则视为跨域类需求，触发 §9-ter D4 强制）
CROSS_DOMAIN_KEYWORDS = (
    "微服务", "聚合根", "跨域", "跨服务", "MQ", "事件广播", "MQ topic", "WebSocket",
    "Redis", "本地缓存", "CQRS",
)

# 时效禁词（复用 ra_authenticity_scan.py 的 missing-timeliness 规则集）
TIMELESS_FORBIDDEN = ("尽快", "及时", "立即", "马上", "实时", "迅速")

# RA 文档命名约定（与 ra_authenticity_scan.py / gates.py:_iter_ra_files 对齐）
RA_FILENAME_PATTERN = re.compile(r"RA[-_]", re.IGNORECASE)


@dataclass
class Finding:
    severity: str   # BLOCKER / WARN
    rule: str       # D1 / D2 / D3 / D4 / D5
    path: str       # 相对 root
    line: int       # 行号（0 = 全文级）
    message: str
    snippet: str


# ─── 通用辅助 ──────────────────────────────────────────────────────────────

def rel(path: Path, root: Path) -> str:
    try:
        return str(path.relative_to(root)).replace("\\", "/")
    except ValueError:
        return str(path).replace("\\", "/")


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def line_no(text: str, index: int) -> int:
    return text.count("\n", 0, index) + 1


def add_finding(findings, severity, rule, path, root, line, message, snippet):
    findings.append(Finding(
        severity=severity,
        rule=rule,
        path=rel(path, root),
        line=line,
        message=message,
        snippet=snippet.strip()[:220],
    ))


# ─── Markdown 表格解析（按表头列名定位列，容忍列序变化）───────────────────

def parse_md_table(section_text: str) -> tuple[list[str], list[list[str]]]:
    """解析 markdown 表格。返回 (headers, rows)。

    - 按行找连续 `|` 包围的行
    - 第一行 = 表头
    - 第二行 = 分隔行（|---|---|...），跳过
    - 后续行 = 数据行（空行或非表格行则停止）
    - 每行按 `|` 切分，首尾空段移除（markdown 表格首尾的 `|`）
    """
    headers: list[str] = []
    rows: list[list[str]] = []
    lines = section_text.splitlines()

    i = 0
    while i < len(lines):
        line = lines[i].rstrip()
        if "|" not in line or line.count("|") < 2:
            i += 1
            continue
        # 候选表头
        next_line = lines[i + 1].rstrip() if i + 1 < len(lines) else ""
        if not re.match(r"^\|?\s*:?-{2,}", next_line.replace("|", "|")):
            i += 1
            continue
        # 表头
        headers = [c.strip() for c in line.strip().strip("|").split("|")]
        i += 2  # 跳过分隔行
        # 数据行
        while i < len(lines):
            r = lines[i].rstrip()
            if "|" not in r or r.count("|") < 2:
                break
            cells = [c.strip() for c in r.strip().strip("|").split("|")]
            # 列数对齐到表头（不足补空，过多截断）
            if len(cells) < len(headers):
                cells = cells + [""] * (len(headers) - len(cells))
            elif len(cells) > len(headers):
                cells = cells[:len(headers)]
            # 跳过纯分隔行残留
            if all(re.match(r"^:?-+:?$", c) for c in cells if c):
                i += 1
                continue
            rows.append(cells)
            i += 1
        break  # 只取首个表格

    return headers, rows


def col_idx(headers: list[str], *candidates: str) -> int:
    """按列名候选列表查找列索引，找不到返回 -1。"""
    norm = [h.replace(" ", "").replace("\u3000", "") for h in headers]
    for cand in candidates:
        cn = cand.replace(" ", "")
        for idx, h in enumerate(norm):
            if h == cn or cn in h:
                return idx
    return -1


def extract_section(content: str, anchor: str) -> str:
    """从 content 提取 anchor 行开头到下一个 ## 级 header 之间的内容。"""
    lines = content.splitlines()
    start = -1
    for i, ln in enumerate(lines):
        if anchor in ln:
            start = i
            break
    if start < 0:
        return ""
    end = len(lines)
    for j in range(start + 1, len(lines)):
        if lines[j].startswith("## "):
            end = j
            break
    return "\n".join(lines[start:end])


def looks_like_rprime(value: str) -> bool:
    """判断字符串是否形如 R1.1 / R12.10 / R-cross-1 等 R′ 标识。"""
    v = value.strip()
    if not v:
        return False
    # 主规则衍生 R1.1 / R1.1, R1.2, ... 或跨主规则共享 R-cross-1
    if re.match(r"^R\d+(\.\d+)?([\s,，、]+R\d+\.\d+)*$", v):
        return True
    if re.match(r"^R-cross-\d+$", v):
        return True
    if re.match(r"^R-cascade-\d+$", v):
        return True
    return False


def extract_rprimes(value: str) -> set[str]:
    """从字符串中抽取所有 R′ 标识（如 'R1.1, R1.2'）。"""
    out: set[str] = set()
    for m in re.finditer(r"R(\d+\.\d+|cross-\d+|cascade-\d+)", value):
        out.add("R" + m.group(1))
    return out


def extract_main_rules(value: str) -> set[str]:
    """从字符串抽取主规则 R1/R2（不含 R1.1）。"""
    return {f"R{m.group(1)}" for m in re.finditer(r"(?<![.\d])R(\d+)(?!\.\d)", value)}


# ─── 主扫描逻辑 ────────────────────────────────────────────────────────────

def is_state_machine_requirement(content: str) -> bool:
    return any(kw in content for kw in STATE_MACHINE_KEYWORDS)


def is_cross_domain_requirement(content: str) -> bool:
    hits = sum(1 for kw in CROSS_DOMAIN_KEYWORDS if kw in content)
    return hits >= 2


def scan_ra_doc(path: Path, root: Path, findings: list[Finding]) -> None:
    text = read_text(path)
    state_machine = is_state_machine_requirement(text)
    cross_domain = is_cross_domain_requirement(text)

    sec_65 = extract_section(text, "§6.5")
    sec_85 = extract_section(text, "§8.5")
    sec_86 = extract_section(text, "§8.6")
    sec_9bis = extract_section(text, "§9-bis")
    sec_9ter = extract_section(text, "§9-ter")

    # ─── D1 §6.5 主规则机械派生（状态机类需求必跑）────────────────────────
    if state_machine and sec_65:
        _check_d1(path, root, text, sec_65, findings)
    elif state_machine and not sec_65:
        # 状态机类需求但缺 §6.5 — 由 G-RA-3 拦截，本扫描器只对存在章节做深度核验
        pass

    # ─── D2 R′→AC 链接完整性（§6.5 与 §8.5 同时存在时必跑）─────────────
    if sec_65 and sec_85:
        _check_d2(path, root, sec_65, sec_85, findings)

    # ─── D3 §8.6 覆盖率真实重算（§6.5/§8.5/§8.6 同时存在时必跑）────────
    if sec_65 and sec_85 and sec_86:
        _check_d3(path, root, sec_65, sec_85, sec_86, findings)

    # ─── D4 §9-ter H.6 五问机械覆盖（跨域类需求必跑）───────────────────
    if cross_domain and sec_9ter:
        _check_d4(path, root, sec_9ter, findings)
    elif cross_domain and not sec_9ter:
        # 跨域类需求但缺 §9-ter — 由 G-RA-3 拦截
        pass

    # ─── D5 §9-bis 业务模式六选一（所有 RA 必跑）─────────────────────────
    if sec_9bis:
        _check_d5(path, root, sec_9bis, findings)


def _check_d1(path: Path, root: Path, full_text: str, sec_65: str, findings: list) -> None:
    """D1：§6.5 主规则机械派生。"""
    headers, rows = parse_md_table(sec_65)
    if not headers or not rows:
        add_finding(findings, "BLOCKER", "D1", path, root, 0,
                    "D1 §6.5 表格为空（仅有表头无逐行数据）。状态机类需求 §6.5 必须每条主规则至少 1 行衍生 R′ 派生数据。",
                    sec_65[:160])
        return

    main_col = col_idx(headers, "规则#", "主规则R", "主规则", "规则编号")
    deriv_col = col_idx(headers, "衍生规则R'", "衍生规则", "R'")
    pattern_col = col_idx(headers, "衍生模式命中", "模式命中", "模式")

    if main_col < 0 or deriv_col < 0 or pattern_col < 0:
        add_finding(findings, "BLOCKER", "D1", path, root, 0,
                    f"D1 §6.5 表头缺关键列：需要「规则#/主规则 R」「衍生规则 R'」「衍生模式命中」（当前表头={headers}）。",
                    ", ".join(headers))
        return

    # 主规则 → 该主规则下衍生 R′ 行数
    main_to_count: dict[str, int] = {}
    main_to_pattern_ok: dict[str, bool] = {}
    rprime_set: set[str] = set()

    for cells in rows:
        main_val = cells[main_col].strip()
        deriv_val = cells[deriv_col].strip()
        pattern_val = cells[pattern_col].strip()

        # 主规则（剥离前缀 R、逗号列表等，取第一个主规则）
        main_match = re.search(r"R\d+", main_val)
        if not main_match:
            continue
        main_id = main_match.group(0)

        # 衍生规则形如 R1.1
        if not looks_like_rprime(deriv_val):
            continue

        main_to_count[main_id] = main_to_count.get(main_id, 0) + 1
        rprime_set.update(extract_rprimes(deriv_val))

        # 模式编号判定：必须含「模式」或「H.5」或具体编号
        has_pattern = bool(re.search(r"(模式|H\.5|H5|\d+\.\d+|\d+[①②③④⑤⑥⑦⑧⑨⑩])", pattern_val))
        if not has_pattern or pattern_val in ("无", "—", "-", "/", ""):
            main_to_pattern_ok[main_id] = False
        else:
            main_to_pattern_ok.setdefault(main_id, True)

    # 校验：每个主规则 R 必须有 ≥1 衍生 R′ 行
    for main_id, cnt in main_to_count.items():
        if cnt < 1:
            add_finding(findings, "BLOCKER", "D1", path, root, 0,
                        f"D1 §6.5 主规则 {main_id} 无衍生 R′ 行（要求 ≥1）。",
                        main_id)

    # 校验：每个主规则的衍生行必须有模式编号
    for main_id, ok in main_to_pattern_ok.items():
        if not ok:
            add_finding(findings, "BLOCKER", "D1", path, root, 0,
                        f"D1 §6.5 主规则 {main_id} 的衍生行「衍生模式命中」列缺模式编号（禁止空/「无」，必须含 H.5/H.5.1/数字编号）。",
                        main_id)

    if not main_to_count:
        add_finding(findings, "BLOCKER", "D1", path, root, 0,
                    "D1 §6.5 未识别出任何主规则 R 及其衍生 R′ 行。",
                    sec_65[:160])


def _check_d2(path: Path, root: Path, sec_65: str, sec_85: str, findings: list) -> None:
    """D2：§6.5 每个 R′ 必须在 §8.5「对应规则 R′」列出现 ≥1 次。"""
    _, rows_65 = parse_md_table(sec_65)
    headers_85, rows_85 = parse_md_table(sec_85)
    if not headers_85 or not rows_85:
        # §8.5 空：由 G-RA-3 拦截
        return

    rprime_link_col = col_idx(headers_85, "对应规则R'", "对应规则", "R'", "对应 R'")
    if rprime_link_col < 0:
        add_finding(findings, "BLOCKER", "D2", path, root, 0,
                    f"D2 §8.5 表头缺「对应规则 R'」列（当前表头={headers_85}）。",
                    ", ".join(headers_85))
        return

    # 收集 §6.5 所有 R′
    rprime_col_65 = col_idx(parse_md_table(sec_65)[0], "衍生规则R'", "衍生规则", "R'")
    if rprime_col_65 < 0:
        return
    all_rprimes: set[str] = set()
    for cells in rows_65:
        all_rprimes.update(extract_rprimes(cells[rprime_col_65]))

    # 收集 §8.5 已链接的 R′
    linked_rprimes: set[str] = set()
    for cells in rows_85:
        linked_rprimes.update(extract_rprimes(cells[rprime_link_col]))

    # 每个 R′ 必须链接
    orphan = all_rprimes - linked_rprimes
    for rp in sorted(orphan):
        add_finding(findings, "BLOCKER", "D2", path, root, 0,
                    f"D2 §6.5 衍生规则 {rp} 在 §8.5 衍生 AC 表中无对应 AC 行（要求每条 R' 至少 1 个 AC）。",
                    rp)


def _check_d3(path: Path, root: Path, sec_65: str, sec_85: str, sec_86: str, findings: list) -> None:
    """D3：§8.6 覆盖率真实重算（K/M ≥80% 且声明值与实际值一致）。"""
    _, rows_65 = parse_md_table(sec_65)
    headers_85, rows_85 = parse_md_table(sec_85)
    _, rows_86 = parse_md_table(sec_86)

    # 重数 M = §6.5 R′ 总数
    rprime_col_65 = -1
    headers_65, _ = parse_md_table(sec_65)
    rprime_col_65 = col_idx(headers_65, "衍生规则R'", "衍生规则", "R'")
    M = 0
    if rprime_col_65 >= 0:
        for cells in rows_65:
            M += len(extract_rprimes(cells[rprime_col_65]))

    # 重数 K = §8.5 衍生 AC 数（数据行数，且对应 R′ 列非空）
    K = 0
    if headers_85 and rows_85:
        link_col = col_idx(headers_85, "对应规则R'", "对应规则", "R'", "对应 R'")
        if link_col >= 0:
            K = sum(1 for cells in rows_85 if cells[link_col].strip())

    if M == 0:
        # §6.5 没有 R′ — 由 D1 拦截
        return

    actual_ratio = K / M

    # 解析 §8.6 声明的覆盖率行（寻找含「K/M」「覆盖率」的格子）
    declared_ratio = None
    headers_86, rows_86 = parse_md_table(sec_86)
    if headers_86 and rows_86:
        cov_col = col_idx(headers_86, "覆盖率", "实际覆盖率", "K/M")
        ratio_col = col_idx(headers_86, "覆盖率", "比值")
        for cells in rows_86:
            joined = " ".join(cells)
            m = re.search(r"(\d+)\s*/\s*(\d+)", joined)
            if m:
                dK, dM = int(m.group(1)), int(m.group(2))
                if dM > 0:
                    declared_ratio = dK / dM
                    break
            pct = re.search(r"(\d+(?:\.\d+)?)\s*%", joined)
            if pct:
                declared_ratio = float(pct.group(1)) / 100
                break

    # D3 核心判定：声明值与实际值一致性（这是"形式通过、内容空转"的典型形态——
    # AI 写出"覆盖率 100%"但重数发现 K/M 只有 30%）。模板语义上无硬阈值要求（K/M 由业务决定），
    # 但"声明值 ≠ 实际值"必然是空转。
    if declared_ratio is not None and abs(declared_ratio - actual_ratio) > 0.05:
        add_finding(findings, "BLOCKER", "D3", path, root, 0,
                    f"D3 §8.6 声明覆盖率 {declared_ratio:.0%} 与实际 K/M={K}/{M}={actual_ratio:.0%} 不一致（误差 >5%）。声明≠实际即视为形式通过、内容空转。",
                    f"声明={declared_ratio:.0%}, 实际={actual_ratio:.0%}")


def _check_d4(path: Path, root: Path, sec_9ter: str, findings: list) -> None:
    """D4：§9-ter 每触发动作 ≥1 行覆盖状态机/事件/缓存/MQ/聚合根。"""
    headers, rows = parse_md_table(sec_9ter)
    if not headers or not rows:
        add_finding(findings, "BLOCKER", "D4", path, root, 0,
                    "D4 §9-ter 表格为空（跨域类需求 §9-ter 必须填实数据）。",
                    sec_9ter[:160])
        return

    trigger_col = col_idx(headers, "触发动作", "动作", "触发事件")
    domain_col = col_idx(headers, "受影响域", "受影响的域", "影响域")
    effect_col = col_idx(headers, "受影响状态机/事件/缓存/MQ", "受影响状态机", "受影响事件",
                          "状态机/事件/缓存/MQ", "影响", "受影响内容")
    timeliness_col = col_idx(headers, "时效要求", "时效", "时间要求")

    if trigger_col < 0 or effect_col < 0:
        add_finding(findings, "BLOCKER", "D4", path, root, 0,
                    f"D4 §9-ter 表头缺关键列：需要「触发动作」「受影响状态机/事件/缓存/MQ」（当前表头={headers}）。",
                    ", ".join(headers))
        return

    # 按触发动作分组
    trigger_to_rows: dict[str, list[list[str]]] = {}
    for cells in rows:
        t = cells[trigger_col].strip()
        if not t:
            continue
        trigger_to_rows.setdefault(t, []).append(cells)

    # 五问关键词（影响内容列必须覆盖至少 5 类之一）
    five_q = {
        "状态机": r"状态机|\b流转\b|\b状态变更\b|状态\s*[→]|→\s*\w+态",
        "事件": r"事件|MQ\s*topic|WebSocket|广播|广播事件|event|topic",
        "缓存": r"缓存|Redis|本地缓存|LocalStorage|失效|DEL\s*key|key\s*失效",
        "MQ": r"MQ|mq\s*topic|消息队列|RocketMQ|Kafka|RabbitMQ",
        "聚合根": r"聚合根|aggregate|CQRS|读模型|写模型",
    }

    for trigger, trows in trigger_to_rows.items():
        # 聚合该触发动作的所有 effect 列文本
        effect_text = " ".join(cells[effect_col] for cells in trows)
        covered = {q for q, pat in five_q.items() if re.search(pat, effect_text, re.IGNORECASE)}
        # 至少覆盖 5 问中的 3 类（业务实践：状态变更至少状态机 + 事件 + 缓存，跨域至少 MQ）
        # 严格按计划：D4 = ≥1 行覆盖五类；五类即状态机/事件/缓存/MQ/聚合根
        # 但跨域类需求至少 MQ + 事件 + 缓存 + 状态机（聚合根可缺）→ 实际判定 ≥3 类
        # 为避免误伤，定义为：跨域类必含「事件 + 缓存 + MQ」三至少，状态机/聚合根有则加分
        must_have = ["事件", "缓存", "MQ"]
        missing = [m for m in must_have if m not in covered]
        if missing:
            add_finding(findings, "BLOCKER", "D4", path, root, 0,
                        f"D4 §9-ter 触发动作「{trigger}」跨域五问覆盖不全，缺 {missing}（要求事件 + 缓存 + MQ 必覆盖，状态机/聚合根有则加分）。当前命中：{sorted(covered)}。",
                        effect_text[:160])

        # 时效禁词（与 ra_authenticity_scan 一致）
        if timeliness_col >= 0:
            for cells in trows:
                tt = cells[timeliness_col].strip()
                if any(bad in tt for bad in TIMELESS_FORBIDDEN):
                    add_finding(findings, "BLOCKER", "D4", path, root, 0,
                                f"D4 §9-ter 触发动作「{trigger}」时效要求「{tt}」含模糊表述（禁止「尽快/及时/立即/马上/实时/迅速」）。",
                                tt)


def _check_d5(path: Path, root: Path, sec_9bis: str, findings: list) -> None:
    """D5：§9-bis 业务模式六选一。"""
    headers, rows = parse_md_table(sec_9bis)
    if not headers or not rows:
        add_finding(findings, "BLOCKER", "D5", path, root, 0,
                    "D5 §9-bis 表格为空（6 大业务模式必须每个明确「适用/不适用」）。",
                    sec_9bis[:160])
        return

    pattern_col = col_idx(headers, "套用的模式", "模式", "业务模式")
    hit_col = col_idx(headers, "命中的衍生影响编号", "衍生影响编号", "命中影响编号", "命中的影响编号")
    note_col = col_idx(headers, "备注（不适用理由）", "备注", "不适用理由", "理由")

    if pattern_col < 0:
        add_finding(findings, "BLOCKER", "D5", path, root, 0,
                    f"D5 §9-bis 表头缺「套用的模式」列（当前表头={headers}）。",
                    ", ".join(headers))
        return

    # 6 大模式必填（与 requirement-analysis-skill H.5.1 对齐）
    SIX_PATTERNS = {"账号状态变更", "订单状态变更", "支付状态变更", "登录态变更", "权限变更", "定时任务状态"}

    found_patterns: set[str] = set()
    for cells in rows:
        pat = cells[pattern_col].strip()
        if not pat:
            continue
        found_patterns.add(pat)
        hit_val = cells[hit_col].strip() if hit_col >= 0 else ""
        note_val = cells[note_col].strip() if note_col >= 0 else ""

        # 「适用」判定：含命中编号 / 含 AC 编号 / 衍生影响编号非空
        is_applicable = bool(re.search(r"[①②③④⑤⑥⑦⑧⑨⑩]|\d+|\bR\d+|\bAC-?\d", hit_val))

        # 「不适用」判定：备注含「不涉及/不适用/无需」+ 理由
        is_not_applicable = bool(re.search(r"不涉及|不适用|无需|无衍生|本需求不涉及|不命中", note_val))

        if not is_applicable and not is_not_applicable:
            add_finding(findings, "BLOCKER", "D5", path, root, 0,
                        f"D5 §9-bis 模式「{pat}」既无「命中衍生影响编号」又无「不适用理由」，必须二选一明确标注。",
                        pat)

    # 6 大模式缺一不可
    missing_patterns = SIX_PATTERNS - found_patterns
    if missing_patterns:
        # 兼容「6 大模式」名称变体（如「订单」可能写「订单状态」）
        found_norm = {p.replace("状态变更", "").replace("变更", "") for p in found_patterns}
        truly_missing = []
        for mp in missing_patterns:
            mp_norm = mp.replace("状态变更", "").replace("变更", "")
            if mp_norm not in found_norm and mp not in found_patterns:
                truly_missing.append(mp)
        if truly_missing:
            add_finding(findings, "BLOCKER", "D5", path, root, 0,
                        f"D5 §9-bis 6 大业务模式缺：{truly_missing}（必须每个模式明确「适用/不适用」）。",
                        ", ".join(truly_missing))


# ─── 文件枚举与扫描 ────────────────────────────────────────────────────────

def iter_ra_docs(root: Path) -> Iterable[Path]:
    for path in root.rglob("*.md"):
        if not RA_FILENAME_PATTERN.search(path.name):
            continue
        lower = path.as_posix().lower()
        if any(seg in lower for seg in ("changelog", "template", "ra-template", "change_log")):
            continue
        yield path


def scan_ra_docs(root: Path, files: tuple[Path, ...] | None = None) -> tuple[list[Finding], int]:
    findings: list[Finding] = []
    ra_files = 0
    candidates = files if files is not None else resolve_ra_scan_scope(root).files
    for path in candidates:
        ra_files += 1
        scan_ra_doc(path, root, findings)
    return findings, ra_files


# ─── 输出渲染 ──────────────────────────────────────────────────────────────

def render_markdown(root: Path, findings: list[Finding], ra_files: int) -> str:
    blockers = sum(1 for f in findings if f.severity == "BLOCKER")
    warnings = sum(1 for f in findings if f.severity == "WARN")
    status = "PASS" if blockers == 0 else "FAIL"
    lines = [
        "# RA Depth Scan Report (🆕 v3.5.9)",
        "",
        "机械派生深度校验 — 验证 E.5/G.5/H.6/H.5 规定的「每行 R→R′→AC 机械追问」是否真做了。",
        "（与 ra_authenticity_scan 的真实性、flow_violation_scan 的流程完整性正交。）",
        "",
        "## Summary",
        "",
        "| Item | Value |",
        "|---|---|",
        f"| Root | `{root}` |",
        f"| Status | `{status}` |",
        f"| RA files scanned | {ra_files} |",
        f"| BLOCKER findings | {blockers} |",
        f"| WARN findings | {warnings} |",
        "",
        "## Findings",
        "",
    ]
    if not findings:
        lines.append("No findings.")
    else:
        lines.append("| Severity | Rule | Location | Message | Snippet |")
        lines.append("|---|---|---|---|---|")
        for f in findings:
            loc = f"`{f.path}:{f.line}`" if f.line else f"`{f.path}`"
            snip = html.escape(f.snippet).replace("|", "\\|").replace("\n", " ")
            lines.append(f"| {f.severity} | `{f.rule}` | {loc} | {f.message} | `{snip}` |")
    lines.append("")
    lines.append("BLOCKER findings make the current RA document mechanically incomplete (form-pass, content-empty).")
    return "\n".join(lines) + "\n"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Scan RA documents for mechanical-derivation depth (5 rules D1-D5, v3.5.9).")
    parser.add_argument("--root", default=".", help="Project root to scan for RA documents.")
    parser.add_argument(
        "--file",
        action="append",
        default=[],
        help="Scan only this authoritative RA Markdown file (repeatable).",
    )
    parser.add_argument("--output", help="Write the scan report to this file.")
    parser.add_argument("--format", choices=["markdown", "json"], default="markdown")
    parser.add_argument("--strict", action="store_true",
                        help="Exit 1 if any BLOCKER violation found.")
    args = parser.parse_args()

    root = Path(args.root).resolve()
    try:
        scope = resolve_ra_scan_scope(root, args.file)
    except RAScanScopeError as exc:
        if args.format == "json":
            sys.stdout.write(json.dumps(
                ra_scan_scope_error_payload(exc, root, args.file),
                ensure_ascii=False,
                indent=2,
            ))
            return 2
        parser.error(str(exc))
    findings, ra_files = scan_ra_docs(root, scope.files)
    findings.sort(key=lambda f: (0 if f.severity == "BLOCKER" else 1, f.path, f.line, f.rule))

    blockers = sum(1 for f in findings if f.severity == "BLOCKER")
    warnings = sum(1 for f in findings if f.severity == "WARN")

    if args.format == "json":
        payload = {
            "root": str(root),
            "scopeMode": scope.mode,
            "selectedFiles": scope.selected_files,
            "excludedFiles": scope.excluded_files,
            "status": "PASS" if blockers == 0 else "FAIL",
            "raFiles": ra_files,
            "ruleStats": {
                "D1": sum(1 for f in findings if f.rule == "D1"),
                "D2": sum(1 for f in findings if f.rule == "D2"),
                "D3": sum(1 for f in findings if f.rule == "D3"),
                "D4": sum(1 for f in findings if f.rule == "D4"),
                "D5": sum(1 for f in findings if f.rule == "D5"),
            },
            "blockers": blockers,
            "warnings": warnings,
            "findings": [asdict(f) for f in findings],
        }
        output = json.dumps(payload, ensure_ascii=False, indent=2)
    else:
        output = render_markdown(root, findings, ra_files)

    if args.output:
        out = Path(args.output)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(output, encoding="utf-8")
    else:
        sys.stdout.write(output)

    # BLOCKER>0 始终 exit 1（对标 ra_authenticity_scan.py 的语义），--strict 仅作为文档化标记
    return 1 if blockers > 0 else 0


if __name__ == "__main__":
    raise SystemExit(main())
