#!/usr/bin/env python3
"""plugin_content_scan.py — 外挂 SKILL/模板内容安全扫描器（🆕 B4 增强）

目标不是替代 CodeReview，而是把外挂 SKILL 中可静态命中的"危险指令"先变成硬信号，
防止来源不明的外挂（尤其 L2 全局层）被无脑加载执行。

对标 coding_authenticity_scan.py / ra_authenticity_scan.py 的架构与 severity 分级。

检测规则（PC-001 ~ PC-010）：
- PC-001 BLOCKER：无差别删除（rm -rf /、rm -rf ~）
- PC-002 BLOCKER：任意命令执行（os.system、subprocess shell=True）
- PC-003 BLOCKER：远程脚本执行（curl|sh、wget|sh）
- PC-004 WARN   ：门禁绕过语义（跳过 G-、禁止跑 gate）
- PC-005 WARN   ：硬编码密钥/token（password=、secret=、api_key=）
- PC-006 INFO   ：内网 IP（10.x、172.x、192.168.x）
- PC-007 BLOCKER：过度权限（chmod 777、chmod +x /）
- PC-008 WARN   ：绕过检查（git --no-verify、--force）
- PC-009 WARN   ：硬编码产出路径（design/story/be/、.ae-project/assets.md 等越界路径，
                  应由 document_storage.resolve_path 推导）— 🆕 v4.1 路径治理
- PC-010 WARN   ：写产出路径但未声明调用 document-storage（文档级聚合判定）— 🆕 v4.1

分层阻断策略（在 plugin_loader.load_registry 接入，非本文件职责）：
- L2 全局层：BLOCKER 命中 → 阻断加载
- L1 项目层：BLOCKER 命中 → 仅告警不阻断（owner 自负）
- L3 仓库根：跳过扫描（git tracked，PR 审核兜底）

零外部依赖，可在本地 agent 会话与 CI 中运行。
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Optional


# === 单文件大小上限（超出跳过 + 告警，防止恶意超大文件拖垮扫描）===
MAX_SCAN_BYTES = 1 * 1024 * 1024  # 1 MB


@dataclass
class PluginFinding:
    """单条扫描发现。"""
    severity: str          # "BLOCKER" | "WARN" | "INFO"
    rule: str              # 如 "PC-001-dangerous-delete"
    plugin: str            # plugin name（调用方填）
    path: str              # 被扫描文件路径
    line: int              # 行号（1-based；0 = 文件级）
    message: str
    snippet: str = ""      # 命中行内容（脱敏）

    def to_dict(self) -> dict:
        return asdict(self)


@dataclass
class ScanResult:
    """单次扫描汇总。"""
    plugin: str
    path: str
    findings: list = field(default_factory=list)
    skipped: bool = False          # 超大/不可读 → 跳过
    skip_reason: str = ""

    @property
    def blockers(self) -> int:
        return sum(1 for f in self.findings if f.severity == "BLOCKER")

    @property
    def warns(self) -> int:
        return sum(1 for f in self.findings if f.severity == "WARN")

    def to_dict(self) -> dict:
        return {
            "plugin": self.plugin,
            "path": self.path,
            "findings": [f.to_dict() for f in self.findings],
            "skipped": self.skipped,
            "skipReason": self.skip_reason,
            "blockers": self.blockers,
            "warns": self.warns,
        }


# === 检测规则（PC-001 ~ PC-008）
# 每条 = (severity, rule_id, compiled_regex, message)
# 正则按"行匹配"设计（外挂 SKILL 是 markdown/text，按行扫描足够且可定位行号）
LINE_RULES: list[tuple[str, str, "re.Pattern[str]", str]] = [
    (
        "BLOCKER",
        "PC-001-dangerous-delete",
        re.compile(r"rm\s+-rf?\s+(/|~|\$HOME|\*|%|C:\\)"),
        "无差别删除：rm -rf 指向根/家目录/通配符，无可挽回风险。",
    ),
    (
        "BLOCKER",
        "PC-002-arbitrary-command-exec",
        re.compile(r"os\.system\s*\(|subprocess\.(?:call|run|Popen)\s*\([^)]*shell\s*=\s*True|eval\s*\(|exec\s*\("),
        "任意命令执行：os.system / shell=True / eval / exec 可执行任意代码。",
    ),
    (
        "BLOCKER",
        "PC-003-remote-script-exec",
        re.compile(r"(?:curl|wget)[^\|]*\|\s*(?:bash|sh|zsh)\b"),
        "远程脚本执行：curl/wget 管道直接喂给 shell，可执行未知远程代码。",
    ),
    (
        "WARN",
        "PC-004-gate-bypass",
        re.compile(r"(?i)(跳过|skip|忽略|ignore|禁止跑|disable).{0,8}(G-\d+|gate|门禁|门卫)"),
        "门禁绕过：外挂含削弱 ae-sdd 门禁纪律的指令，请人工确认其意图。",
    ),
    (
        "WARN",
        "PC-005-hardcoded-secret",
        re.compile(r"(?i)\b(password|passwd|secret|token|api[_-]?key)\b\s*[:=]\s*[\"'][^\"'${}]{4,}[\"']"),
        "硬编码密钥：外挂含明文 password/secret/token/api_key，疑似凭证泄露。",
    ),
    (
        "INFO",
        "PC-006-internal-ip",
        re.compile(r"\b(?:10\.\d{1,3}\.\d{1,3}\.\d{1,3}|172\.\d{1,3}\.\d{1,3}\.\d{1,3}|192\.168\.\d{1,3}\.\d{1,3})\b"),
        "内网 IP：外挂含内网地址，信息泄露提示（仅 INFO，不阻断）。",
    ),
    (
        "BLOCKER",
        "PC-007-excessive-permission",
        re.compile(r"chmod\s+777\b|chmod\s+\+x\s+/(?:\S+)"),
        "过度权限：chmod 777 / chmod +x / 赋予危险权限。",
    ),
    (
        "WARN",
        "PC-008-check-bypass",
        re.compile(r"git\s+\w+\s+--no-verify\b|git\s+push\s+--force(?:-with-lease)?\b"),
        "绕过检查：git --no-verify / --force 跳过 hook 或覆盖远端。",
    ),
    # 🆕 v4.1 路径治理：硬编码产出路径（应经 document_storage.resolve_path 推导）
    (
        "WARN",
        "PC-009-hardcoded-output-path",
        re.compile(
            r"(?:design/story/be/|design/testcase/be/|\.ae-project/assets\.md"
            r"|life-team-project-docs/|\.ae-task/|\.ae-plan/|\.spec/iterations/)"
        ),
        "硬编码产出路径：检测到 deprecated/越界路径（design/、.ae-project/ 等），"
        "应由 document_storage.resolve_path 推导，不得硬编码（见 document-storage §0.6.1）。",
    ),
]


def scan_plugin_file(path: Path, plugin_name: str = "") -> ScanResult:
    """扫描单个外挂文件，返回 ScanResult。

    失败优先：文件不可读 / 过大 → skipped=True，不抛异常（调用方按告警处理）。
    """
    result = ScanResult(plugin=plugin_name, path=str(path))

    if not path.is_file():
        result.skipped = True
        result.skip_reason = "文件不存在"
        return result

    try:
        size = path.stat().st_size
    except OSError:
        result.skipped = True
        result.skip_reason = "无法读取文件大小"
        return result

    if size > MAX_SCAN_BYTES:
        result.skipped = True
        result.skip_reason = f"文件超 {MAX_SCAN_BYTES // 1024 // 1024}MB 上限，跳过（防 DoS）"
        return result

    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError as e:
        result.skipped = True
        result.skip_reason = f"读取失败：{e}"
        return result

    for lineno, line in enumerate(text.splitlines(), start=1):
        for severity, rule_id, pattern, message in LINE_RULES:
            if pattern.search(line):
                # 脱敏：截断长行 + 去掉首尾空白
                snippet = line.strip()[:120]
                result.findings.append(PluginFinding(
                    severity=severity,
                    rule=rule_id,
                    plugin=plugin_name,
                    path=str(path),
                    line=lineno,
                    message=message,
                    snippet=snippet,
                ))

    # 🆕 v4.1 PC-010 文档级聚合判定：写产出路径但未声明调用 document-storage
    _check_pc010_missing_doc_storage_call(text, path, plugin_name, result)

    return result


# ─── PC-010 文档级检测（非逐行，聚合判定）──────────────────────────────────
# 产出路径线索：ae-sdd-doc/ 下子目录、或 SKILL 里出现落地路径模板
_PC010_PATH_HINT_RE = re.compile(
    r"ae-sdd-doc/(?:PRD|RA|DR|Story|Task|Coding|Test|CR)/"
    r"|复制本文到\s+\S+/|存放路径[：:]\s*`?\S+/"
)
# document-storage 声明线索：API 名、或引用 document-storage-skill
_PC010_DECLARATION_RE = re.compile(
    r"document[_-]?storage|resolve_path|save_doc|choose_iteration"
    r"|document-storage-skill",
    re.IGNORECASE,
)


def _check_pc010_missing_doc_storage_call(text: str, path: Path,
                                          plugin_name: str, result: ScanResult) -> None:
    """PC-010：文档含产出路径线索但无 document-storage 声明 → WARN。

    判定：全文有 _PC010_PATH_HINT_RE 命中（≥1 处产出路径）且
          无 _PC010_DECLARATION_RE 命中（无 document-storage 调用/引用）。
    线索 0 处（非产出文档）或已声明 → 不报。
    """
    if not _PC010_PATH_HINT_RE.search(text):
        return  # 无产出路径线索，不是产出文档，跳过
    if _PC010_DECLARATION_RE.search(text):
        return  # 已声明 document-storage，合规
    # 未声明 → 告警（定位到首个产出路径行）
    for lineno, line in enumerate(text.splitlines(), start=1):
        if _PC010_PATH_HINT_RE.search(line):
            result.findings.append(PluginFinding(
                severity="WARN",
                rule="PC-010-missing-doc-storage-call",
                plugin=plugin_name,
                path=str(path),
                line=lineno,
                message="未声明调用 document-storage：文档定义了产出路径但无 "
                        "resolve_path/save_doc 声明，应委派 document-storage-skill 推导路径。",
                snippet=line.strip()[:120],
            ))
            break


def has_blocker(result: ScanResult) -> bool:
    """判断扫描结果是否含 BLOCKER（供 plugin_loader 分层阻断用）。"""
    return result.blockers > 0


# === CLI（独立可用，亦可被 ae-sdd CLI 调用）===

def _format_finding(f: PluginFinding) -> str:
    icon = {"BLOCKER": "🔴", "WARN": "🟡", "INFO": "🔵"}.get(f.severity, "•")
    return (f"  {icon} {f.severity} {f.rule} @ L{f.line}\n"
            f"     {f.message}\n"
            f"     | {f.snippet}")


def main(argv: Optional[list] = None) -> int:
    parser = argparse.ArgumentParser(
        prog="plugin_content_scan",
        description="外挂 SKILL/模板内容安全扫描器（B4 增强）。扫描单个文件的危险指令。",
    )
    parser.add_argument("path", help="待扫描的外挂文件路径")
    parser.add_argument("--plugin", default="", help="plugin name（用于结果标识）")
    parser.add_argument("--json", action="store_true", help="输出 JSON")
    args = parser.parse_args(argv)

    result = scan_plugin_file(Path(args.path), args.plugin)

    if args.json:
        print(json.dumps(result.to_dict(), ensure_ascii=False, indent=2))
    else:
        if result.skipped:
            print(f"⊘ 跳过扫描：{result.plugin or args.path}（{result.skip_reason}）")
        else:
            print(f"🔍 扫描：{result.plugin or args.path}")
            for f in result.findings:
                print(_format_finding(f))
            print(f"\n汇总：{result.blockers} BLOCKER / {result.warns} WARN / "
                  f"{len(result.findings) - result.blockers - result.warns} INFO")

    # 退出码：有 BLOCKER → 1，否则 0（skipped 不算失败）
    return 1 if has_blocker(result) else 0


if __name__ == "__main__":
    sys.exit(main())
