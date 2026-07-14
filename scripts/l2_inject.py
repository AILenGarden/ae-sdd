#!/usr/bin/env python3
"""
l2_inject.py - ae-sdd L2 会话级纪律 SSOT 注入器（🆕 v3.10.8）。

把 `source/L2-DISCIPLINE.md`（SSOT 母版）按 agent 语言切片，注入到各 agent
全局指令文件（ZCode AGENTS.md / Codex AGENTS.md / Claude CLAUDE.md）的锚点区间。

核心安全约束：**锚点外零改动**。注入器只替换 `BEGIN`/`END` 锚点之间的内容，
落盘后用 difflib 校验锚点外区域与备份字节一致，不一致则回滚并报错。

三种模式：
  - 常规注入（post-commit 自动链路）：仅对已有锚点的 agent 做区间替换
  - bootstrap --dry-run：首次落盘预览（识别现有 ae-sdd 段落边界 + diff）
  - bootstrap --apply：首次落盘执行（替换现有段落为锚点包裹版）

用法:
    python scripts/l2_inject.py                         # 常规注入（已 bootstrap 的 agent）
    python scripts/l2_inject.py --target claude         # 指定单 agent
    python scripts/l2_inject.py --bootstrap --dry-run   # 首次落盘预览
    python scripts/l2_inject.py --bootstrap --apply     # 首次落盘执行
    python scripts/l2_inject.py --rollback zcode        # 回滚到最近备份
    python scripts/l2_inject.py --quiet                 # 静默（post-commit 用）
"""
from __future__ import annotations

import argparse
import difflib
import hashlib
import re
import shutil
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

if hasattr(sys.stdout, "reconfigure"):
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stderr.reconfigure(encoding="utf-8")

# 让 `from tools.lib ...` 可用（与 distribute.py 同栈）
_REPO_ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(_REPO_ROOT))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from tools.lib.distributor_registry import load_registry, DistributorEntry  # noqa: E402

# ─── 常量 ─────────────────────────────────────────────────────────────────────

SSOT_PATH = _REPO_ROOT / "source" / "L2-DISCIPLINE.md"

ANCHOR_BEGIN = "ae-sdd-l2-ssot"
ANCHOR_BEGIN_RE = re.compile(r"<!--\s*BEGIN\s+ae-sdd-l2-ssot\b.*?-->")
ANCHOR_END_RE = re.compile(r"<!--\s*END\s+ae-sdd-l2-ssot\s*-->")

SSOT_VERSION = "1.0.0"  # 适配器版本（变更注入逻辑时 bump，触发全量重注入）

MAX_BACKUPS = 3

# ─── 各 agent bootstrap 段落边界识别模式 ──────────────────────────────────────
# key = agent name; value = (start_pattern, end_strategy)
#   start_pattern: 匹配现有 ae-sdd 段落起始标题行
#   end_strategy: "next_h2" = 到下一个 ## 标题（不含）；"next_h2_or_hr" = 到下一个
#                 ## 标题或独立的 --- 分隔线
# 覆盖三家当前手写的 ae-sdd 段落变体。
_BOOTSTRAP_PATTERNS: dict[str, list[tuple[str, str]]] = {
    "zcode": [
        # ZCode: 两段连续（工作流强制调用 + Coding 执行纪律），中间有 ---
        (r"^## ae-sdd 工作流强制调用", "next_h2_design"),
    ],
    "codex": [
        (r"^## Mandatory ae-sdd Coding Workflow", "next_h2"),
    ],
    "claude": [
        # Claude: 工作流调用 + 编码行为约束表格（可能连续）
        (r"^## ae-sdd 工作流调用", "next_h2_design"),
    ],
}


# ─── 数据类 ───────────────────────────────────────────────────────────────────

@dataclass
class InjectResult:
    agent: str
    status: str  # ok | skip | skip_no_anchor | fail
    message: str
    target_file: Optional[Path] = None


# ─── SSOT 读取与语言切片 ─────────────────────────────────────────────────────

def _read_ssot() -> str:
    """读取 SSOT 母版全文。"""
    if not SSOT_PATH.is_file():
        raise FileNotFoundError(f"SSOT 母版不存在: {SSOT_PATH}")
    return SSOT_PATH.read_text(encoding="utf-8")


def _slice_section(ssot: str, lang: str) -> str:
    """从 SSOT 切出指定语言的纪律段（SECTION:zh 或 SECTION:en）。

    返回 section 标记之间的原始文本（不含标记注释行）。
    用正则匹配，容忍 markdown 编辑器把 `--` 替换成 typographic dash 的情况。
    """
    open_re = re.compile(rf"<!--\s*SECTION:{lang}\b.*?-->", re.DOTALL)
    close_re = re.compile(rf"<!--\s*/SECTION:{lang}\s*-->")
    m_open = open_re.search(ssot)
    if not m_open:
        raise ValueError(f"SSOT 中找不到 SECTION:{lang} 标记")
    # 跳过标记所在行（到下一个换行）
    line_end = ssot.find("\n", m_open.end())
    if line_end == -1:
        raise ValueError(f"SECTION:{lang} 标记后无换行")
    m_close = close_re.search(ssot, line_end)
    if not m_close:
        raise ValueError(f"SSOT 中找不到 SECTION:{lang} 闭合标记")
    body = ssot[line_end + 1:m_close.start()]
    # 去掉尾部空行
    return body.rstrip() + "\n"


def _slice_redline11(ssot: str, lang: str) -> str:
    """从 SSOT 切出红线条款 11 的语言版本（redline11:zh / redline11:en）。

    返回表格行 + 同源注释（用于 Claude bootstrap 时补进红线表）。
    """
    open_re = re.compile(rf"<!--\s*redline11:{lang}\s*-->")
    close_re = re.compile(rf"<!--\s*/redline11:{lang}\s*-->")
    m_open = open_re.search(ssot)
    if not m_open:
        raise ValueError(f"SSOT 中找不到 redline11:{lang} 标记")
    line_end = ssot.find("\n", m_open.end())
    if line_end == -1:
        line_end = m_open.end()
    m_close = close_re.search(ssot, line_end)
    if not m_close:
        raise ValueError(f"SSOT 中找不到 redline11:{lang} 闭合标记")
    body = ssot[line_end + 1:m_close.start()]
    return body.strip()


def _content_hash(text: str) -> str:
    """计算文本的短 hash（用于 diff-aware skip）。"""
    return hashlib.sha256(text.encode("utf-8")).hexdigest()[:12]


def _git_commit() -> str:
    """获取当前仓库的短 commit hash（注入审计头用）。失败返回 unknown。"""
    try:
        import subprocess
        out = subprocess.check_output(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=_REPO_ROOT, stderr=subprocess.DEVNULL, timeout=5,
        ).decode().strip()
        return out or "unknown"
    except Exception:
        return "unknown"


def _utc_stamp() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def _render_anchor_block(lang: str) -> str:
    """渲染完整的锚点区间内容（BEGIN ... END），含审计头 + SSOT 切片。"""
    ssot = _read_ssot()
    body = _slice_section(ssot, lang)
    commit = _git_commit()
    ts = _utc_stamp()
    h = _content_hash(body)
    begin = f"<!-- BEGIN {ANCHOR_BEGIN} @ {commit} @ {ts} (hash={h} v={SSOT_VERSION}) -->"
    end = f"<!-- END {ANCHOR_BEGIN} -->"
    return f"{begin}\n{body}\n{end}\n"


# ─── 锚点定位 ─────────────────────────────────────────────────────────────────

@dataclass
class AnchorSpan:
    """锚点区间在文件中的位置。

    before: BEGIN 行之前的内容（含 BEGIN 行）
    inner: BEGIN 与 END 之间的内容
    after: END 行及之后的内容
    """
    begin_line_idx: int   # BEGIN 行的行索引
    end_line_idx: int     # END 行的行索引
    inner_lines: list[str]


def _find_anchor(lines: list[str]) -> Optional[AnchorSpan]:
    """在行列表中定位锚点区间。无锚点返回 None。"""
    begin_idx = None
    end_idx = None
    for i, line in enumerate(lines):
        if begin_idx is None and ANCHOR_BEGIN_RE.search(line):
            begin_idx = i
        elif begin_idx is not None and ANCHOR_END_RE.search(line):
            end_idx = i
            break
    if begin_idx is None or end_idx is None:
        return None
    return AnchorSpan(
        begin_line_idx=begin_idx,
        end_line_idx=end_idx,
        inner_lines=lines[begin_idx + 1:end_idx],
    )


# ─── 备份与回滚 ───────────────────────────────────────────────────────────────

def _backup_file(path: Path) -> Optional[Path]:
    """备份文件到 <path>.bak.<ts>，保留最近 MAX_BACKUPS 份。"""
    if not path.is_file():
        return None
    ts = _utc_stamp()
    bak = path.with_name(f"{path.name}.bak.{ts}")
    shutil.copy2(path, bak)
    # 清理旧备份
    baks = sorted(path.parent.glob(f"{path.name}.bak.*"))
    if len(baks) > MAX_BACKUPS:
        for old in baks[:-MAX_BACKUPS]:
            old.unlink(missing_ok=True)
    return bak


def _latest_backup(path: Path) -> Optional[Path]:
    """获取最近一份备份。"""
    baks = sorted(path.parent.glob(f"{path.name}.bak.*"))
    return baks[-1] if baks else None


def _rollback(path: Path) -> bool:
    """回滚到最近备份。成功返回 True。"""
    bak = _latest_backup(path)
    if bak is None:
        return False
    shutil.copy2(bak, path)
    return True


# ─── 锚点外零改动校验 ─────────────────────────────────────────────────────────

def _extract_outside_anchor(lines: list[str], span: AnchorSpan) -> list[str]:
    """提取锚点区间之外的行（before + after，不含锚点行本身）。"""
    before = lines[:span.begin_line_idx]
    after = lines[span.end_line_idx + 1:]
    return before + after


def _verify_outside_unchanged(
    original_outside: list[str], new_outside: list[str]
) -> tuple[bool, str]:
    """校验锚点外区域字节一致。返回 (一致, diff描述)。"""
    if original_outside == new_outside:
        return True, ""
    diff = difflib.unified_diff(
        original_outside, new_outside,
        fromfile="original-outside", tofile="new-outside",
        lineterm="",
    )
    return False, "\n".join(diff)


# ─── 常规注入（锚点区间替换）─────────────────────────────────────────────────

def _inject_anchored(path: Path, lang: str, quiet: bool) -> InjectResult:
    """对已有锚点的文件做区间替换。"""
    original_text = path.read_text(encoding="utf-8")
    original_lines = original_text.splitlines(keepends=True)
    span = _find_anchor(original_lines)

    if span is None:
        return InjectResult(
            agent="", status="skip_no_anchor",
            message=f"无锚点，跳过（需先 bootstrap）：{path}",
            target_file=path,
        )

    new_block = _render_anchor_block(lang)
    new_lines = (
        original_lines[:span.begin_line_idx]
        + [line if line.endswith("\n") else line + "\n" for line in new_block.splitlines(keepends=True)]
        + original_lines[span.end_line_idx + 1:]
    )

    # diff-aware skip：若区间内容 hash 已一致则跳过
    old_inner_hash = _content_hash("".join(span.inner_lines))
    new_inner_text = new_block  # 含 BEGIN/END
    # 提取新区间 inner hash（去掉 BEGIN/END 行）
    new_block_lines = new_block.splitlines(keepends=True)
    new_inner = new_block_lines[1:-1] if len(new_block_lines) >= 3 else []
    new_inner_hash = _content_hash("".join(new_inner))
    if old_inner_hash == new_inner_hash:
        return InjectResult(
            agent="", status="skip",
            message=f"区间内容 hash 一致，跳过：{path}",
            target_file=path,
        )

    # 备份
    bak = _backup_file(path)

    # 落盘
    path.write_text("".join(new_lines), encoding="utf-8")

    # 校验锚点外零改动
    new_text = path.read_text(encoding="utf-8")
    new_lines_read = new_text.splitlines(keepends=True)
    new_span = _find_anchor(new_lines_read)
    if new_span is None:
        _rollback(path)
        return InjectResult(agent="", status="fail",
                            message=f"落盘后锚点丢失，已回滚：{path}", target_file=path)
    original_outside = _extract_outside_anchor(original_lines, span)
    new_outside = _extract_outside_anchor(new_lines_read, new_span)
    ok, diff = _verify_outside_unchanged(original_outside, new_outside)
    if not ok:
        _rollback(path)
        return InjectResult(
            agent="", status="fail",
            message=f"锚点外区域被改动，已回滚：{path}\n{diff}",
            target_file=path,
        )

    return InjectResult(
        agent="", status="ok",
        message=f"已更新锚点区间：{path}（备份: {bak.name if bak else 'N/A'}）",
        target_file=path,
    )


# ─── Bootstrap（首次锚点落盘）────────────────────────────────────────────────

def _find_bootstrap_span(lines: list[str], agent: str) -> Optional[tuple[int, int]]:
    """识别现有 ae-sdd 段落的行范围 [start, end)。

    返回 (start_line_idx, end_line_idx)，end 是排他的（即 end 行不被替换）。
    """
    patterns = _BOOTSTRAP_PATTERNS.get(agent, [])
    for start_pat, end_strategy in patterns:
        start_re = re.compile(start_pat)
        start_idx = None
        for i, line in enumerate(lines):
            if start_re.match(line):
                start_idx = i
                break
        if start_idx is None:
            continue
        # 找结束位置
        end_idx = len(lines)
        for j in range(start_idx + 1, len(lines)):
            line = lines[j]
            if end_strategy == "next_h2":
                if line.startswith("## "):
                    end_idx = j
                    break
            elif end_strategy == "next_h2_design":
                # 到下一个 ## 标题，但跳过 ### 子标题和 --- 分隔线
                # ae-sdd 两段之间有 --- 分隔，要包含进去，直到真正的下一个 ## 顶级标题
                if line.startswith("## ") and not line.startswith("### "):
                    # 确认不是 ae-sdd 自身的第二段（ZCode 有两段连续）
                    if "ae-sdd" in line.lower() or "Coding 执行纪律" in line:
                        continue  # 继续包含 ae-sdd 的后续段落
                    end_idx = j
                    break
            # 默认：到下一个 ##
            if line.startswith("## ") and not line.startswith("### "):
                if "ae-sdd" in line.lower():
                    continue
                end_idx = j
                break
        else:
            end_idx = len(lines)
        # 回退尾部空行和 --- 分隔线（不替换它们，让锚点块自己结尾）
        while end_idx > start_idx + 1:
            prev = lines[end_idx - 1].strip()
            if prev == "" or prev == "---":
                end_idx -= 1
            else:
                break
        return (start_idx, end_idx)
    return None


def _find_claude_redline11_gap(lines: list[str]) -> Optional[int]:
    """Claude bootstrap 专用：定位红线表条款 10 行，返回条款 11 应插入的行索引。

    Claude 红线表当前 10 条，需在条款 10 行后插入条款 11 行 + 同源注释。
    返回插入点（条款 10 行的索引 + 1）。找不到返回 None。
    """
    for i, line in enumerate(lines):
        # 匹配 "| 10 |" 行
        if re.match(r"^\|\s*10\s*\|", line):
            return i + 1
    return None


def _bootstrap_preview(
    path: Path, agent: str, lang: str
) -> dict:
    """生成 bootstrap dry-run 预览。返回结构化预览数据。"""
    original_text = path.read_text(encoding="utf-8")
    original_lines = original_text.splitlines(keepends=True)

    span = _find_bootstrap_span(original_lines, agent)
    if span is None:
        return {
            "found": False,
            "message": f"未识别到现有 ae-sdd 段落（agent={agent}）",
        }
    start_idx, end_idx = span
    old_segment = "".join(original_lines[start_idx:end_idx])

    new_block = _render_anchor_block(lang)
    new_lines_preview = (
        original_lines[:start_idx]
        + [l + "\n" if not l.endswith("\n") else l for l in new_block.splitlines()]
        + original_lines[end_idx:]
    )
    new_segment_preview = new_block

    # Claude 红线条款 11 补丁预览
    redline_patch = None
    if agent == "claude":
        gap = _find_claude_redline11_gap(original_lines)
        if gap is not None:
            rl11 = _slice_redline11(_read_ssot(), lang)
            redline_patch = {
                "insert_at_line": gap,
                "content": rl11,
                "note": "Claude 红线表当前 10 条，补齐条款 11 + 同源注释（锚点区外，单独标记）",
            }

    # 生成 diff
    diff = list(difflib.unified_diff(
        original_lines, new_lines_preview,
        fromfile=f"{path.name} (original)", tofile=f"{path.name} (after bootstrap)",
        lineterm="",
    ))

    return {
        "found": True,
        "replace_range": (start_idx + 1, end_idx),  # 1-based for display
        "old_segment_preview": old_segment[:500] + ("..." if len(old_segment) > 500 else ""),
        "new_segment_preview": new_segment_preview[:500] + ("..." if len(new_segment_preview) > 500 else ""),
        "redline_patch": redline_patch,
        "diff": "\n".join(diff),
        "outside_unchanged_assertion": "锚点外区域（含红线表主体、技术栈等）零改动",
    }


def _bootstrap_apply(path: Path, agent: str, lang: str, quiet: bool) -> InjectResult:
    """执行 bootstrap 落盘。"""
    original_text = path.read_text(encoding="utf-8")
    original_lines = original_text.splitlines(keepends=True)

    span = _find_bootstrap_span(original_lines, agent)
    if span is None:
        return InjectResult(
            agent=agent, status="fail",
            message=f"未识别到现有 ae-sdd 段落，无法 bootstrap：{path}",
        )
    start_idx, end_idx = span

    new_block = _render_anchor_block(lang)
    new_block_lines = [l + "\n" if not l.endswith("\n") else l for l in new_block.splitlines()]

    # 备份
    bak = _backup_file(path)

    # 组装新文件：替换 ae-sdd 段落为锚点块
    new_lines = (
        original_lines[:start_idx]
        + new_block_lines
        + original_lines[end_idx:]
    )

    # Claude 红线条款 11 补丁（在锚点区外，单独插入）
    if agent == "claude":
        gap = _find_claude_redline11_gap(new_lines)
        if gap is not None:
            rl11 = _slice_redline11(_read_ssot(), lang)
            rl11_lines = [l + "\n" for l in rl11.splitlines()]
            new_lines = new_lines[:gap] + rl11_lines + new_lines[gap:]

    path.write_text("".join(new_lines), encoding="utf-8")

    # 校验：确认锚点存在
    new_text = path.read_text(encoding="utf-8")
    new_lines_read = new_text.splitlines(keepends=True)
    new_span = _find_anchor(new_lines_read)
    if new_span is None:
        _rollback(path)
        return InjectResult(agent=agent, status="fail",
                            message=f"bootstrap 后锚点未生成，已回滚：{path}")

    return InjectResult(
        agent=agent, status="ok",
        message=f"bootstrap 完成：{path}（备份: {bak.name if bak else 'N/A'}）",
        target_file=path,
    )


# ─── 入口 ─────────────────────────────────────────────────────────────────────

def _agent_entries() -> list[DistributorEntry]:
    """返回需要 L2 注入的 agent 条目（有 l2_global_file 且 enabled）。"""
    entries = load_registry()
    return [e for e in entries
            if e.enabled and e.l2_global_file and e.l2_language]


def inject_all(quiet: bool = False) -> list[InjectResult]:
    """常规注入入口（post-commit 自动链路调用）。

    仅对已有锚点的 agent 做区间替换；无锚点的 agent 报 skip_no_anchor（不自动 bootstrap）。
    """
    results = []
    for entry in _agent_entries():
        target = Path(entry.l2_global_file).expanduser()
        if not target.is_file():
            results.append(InjectResult(
                agent=entry.name, status="skip",
                message=f"目标文件不存在，跳过：{target}",
            ))
            continue
        res = _inject_anchored(target, entry.l2_language, quiet)
        res.agent = entry.name
        results.append(res)
        if not quiet:
            _print_result(res)
    return results


def _print_result(res: InjectResult) -> None:
    icons = {"ok": "✅", "skip": "⏭️", "skip_no_anchor": "⚠️", "fail": "❌"}
    icon = icons.get(res.status, "?")
    print(f"  {icon} [{res.agent}] {res.message}")


def main() -> int:
    parser = argparse.ArgumentParser(description="ae-sdd L2 会话级纪律 SSOT 注入器")
    parser.add_argument("--target", help="只处理指定 agent（claude/codex/zcode）")
    parser.add_argument("--quiet", action="store_true", help="静默模式")
    parser.add_argument("--bootstrap", action="store_true", help="首次锚点落盘模式")
    parser.add_argument("--dry-run", action="store_true", help="仅预览，不落盘（配合 --bootstrap）")
    parser.add_argument("--apply", action="store_true", help="执行落盘（配合 --bootstrap）")
    parser.add_argument("--rollback", metavar="AGENT", help="回滚指定 agent 到最近备份")
    args = parser.parse_args()

    # 回滚模式
    if args.rollback:
        entries = _agent_entries()
        entry = next((e for e in entries if e.name == args.rollback), None)
        if entry is None:
            print(f"❌ 未知 agent 或无 l2 配置: {args.rollback}")
            return 1
        target = Path(entry.l2_global_file).expanduser()
        if _rollback(target):
            print(f"✅ 已回滚 {args.rollback}: {target}")
            return 0
        print(f"❌ 无可用备份: {target}")
        return 1

    # Bootstrap 模式
    if args.bootstrap:
        entries = _agent_entries()
        if args.target:
            entries = [e for e in entries if e.name == args.target]
        if not args.dry_run and not args.apply:
            print("❌ bootstrap 必须配合 --dry-run 或 --apply")
            return 1
        any_fail = False
        for entry in entries:
            target = Path(entry.l2_global_file).expanduser()
            if not target.is_file():
                print(f"  ⚠️ [{entry.name}] 目标文件不存在: {target}")
                continue
            if args.dry_run:
                print(f"\n{'='*70}")
                print(f"  Bootstrap 预览: {entry.name} ({entry.l2_language})")
                print(f"  目标: {target}")
                print(f"{'='*70}")
                preview = _bootstrap_preview(target, entry.name, entry.l2_language)
                if not preview["found"]:
                    print(f"  ❌ {preview['message']}")
                    any_fail = True
                    continue
                print(f"  替换行范围: {preview['replace_range'][0]}-{preview['replace_range'][1]}")
                print(f"\n  --- 旧段落预览 ---\n{preview['old_segment_preview']}")
                print(f"\n  --- 新锚点块预览 ---\n{preview['new_segment_preview']}")
                if preview.get("redline_patch"):
                    rp = preview["redline_patch"]
                    print(f"\n  🔴 红线补丁（锚点区外）:")
                    print(f"     插入位置: 第 {rp['insert_at_line']} 行后")
                    print(f"     内容:\n{rp['content']}")
                    print(f"     说明: {rp['note']}")
                print(f"\n  ✅ {preview['outside_unchanged_assertion']}")
                print(f"\n  --- 完整 diff ---\n{preview['diff']}")
            elif args.apply:
                res = _bootstrap_apply(target, entry.name, entry.l2_language, args.quiet)
                _print_result(res)
                if res.status == "fail":
                    any_fail = True
        return 1 if any_fail else 0

    # 常规注入模式
    if args.target:
        entries = _agent_entries()
        entry = next((e for e in entries if e.name == args.target), None)
        if entry is None:
            print(f"❌ 未知 agent 或无 l2 配置: {args.target}")
            return 1
        target = Path(entry.l2_global_file).expanduser()
        if not target.is_file():
            print(f"❌ 目标文件不存在: {target}")
            return 1
        res = _inject_anchored(target, entry.l2_language, args.quiet)
        res.agent = entry.name
        _print_result(res)
        return 0 if res.status in ("ok", "skip", "skip_no_anchor") else 1

    results = inject_all(args.quiet)
    any_fail = any(r.status == "fail" for r in results)
    return 1 if any_fail else 0


if __name__ == "__main__":
    sys.exit(main())
