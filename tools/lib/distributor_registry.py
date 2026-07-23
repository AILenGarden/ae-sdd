"""分发器注册表读写 + 扫描逻辑（🆕 2026-07-03 注册表模式）。

注册表位置：~/.ae-sdd/distributors.json（用户环境态，与 plugins/ 同级）。
首次运行无文件时，从 _default_distributors() 种子初始化。

注册表驱动分发器实例构造：register = 选协议模板 + 填参数；
unregister = 从注册表除名（对应实例不再构造）。
"""
from __future__ import annotations

import json
import shutil
from dataclasses import dataclass, asdict
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional


SCHEMA_VERSION = 1
REGISTRY_FILENAME = "distributors.json"

# 已知 Agent 的默认安装模式（scan 与种子初始化共用）
# name → (protocol, target_path, detect, detect_cli, notes)
_KNOWN_AGENTS: dict[str, dict[str, Any]] = {
    "claude": {
        "protocol": "copytree",
        "target_path": "~/.claude/skills/ae-sdd",
        "detect": "always",
        "detect_cli": None,
        "notes": "Claude Code",
        "l2_global_file": "~/.claude/CLAUDE.md",
        "l2_language": "zh",
    },
    "codex": {
        "protocol": "copytree",
        "target_path": "~/.codex/skills/ae-sdd",
        "detect": "path_exists",
        "detect_cli": None,
        "notes": "OpenAI Codex",
        "l2_global_file": "~/.codex/AGENTS.md",
        "l2_language": "en",
    },
    "zcode": {
        "protocol": "copytree",
        "target_path": "~/.zcode/skills/ae-sdd",
        "detect": "path_exists",
        "detect_cli": None,
        "notes": "ZCode",
        "l2_global_file": "~/.zcode/AGENTS.md",
        "l2_language": "zh",
    },
    "hermes": {
        "protocol": "copytree",
        "target_path": "~/.hermes/skills/ae-sdd",
        "detect": "path_exists",
        "detect_cli": None,
        "notes": "Hermes",
        "l2_global_file": None,
        "l2_language": None,
    },
}


@dataclass
class DistributorEntry:
    """注册表单条目。"""
    name: str
    protocol: str               # copytree | harness_mount
    target_path: str            # 支持 ~ 开头
    detect: str                 # always | path_exists | cli_exists
    detect_cli: Optional[str]   # detect=cli_exists 时填
    enabled: bool = True
    registered_at: str = ""
    notes: str = ""
    l2_global_file: Optional[str] = None   # 🆕 L2 会话级纪律注入目标（None=跳过）
    l2_language: Optional[str] = None      # 🆕 L2 渲染语言 zh|en（None=跳过）

    def to_dict(self) -> dict[str, Any]:
        d = asdict(self)
        # detect_cli 为 None 时省略，保持 JSON 整洁
        if d.get("detect_cli") is None:
            d["detect_cli"] = None
        return d

    @classmethod
    def from_dict(cls, d: dict[str, Any]) -> "DistributorEntry":
        return cls(
            name=str(d.get("name", "")),
            protocol=str(d.get("protocol", "copytree")),
            target_path=str(d.get("target_path", "")),
            detect=str(d.get("detect", "always")),
            detect_cli=d.get("detect_cli"),
            enabled=bool(d.get("enabled", True)),
            registered_at=str(d.get("registered_at", "")),
            notes=str(d.get("notes", "")),
            l2_global_file=d.get("l2_global_file"),
            l2_language=d.get("l2_language"),
        )

    def resolved_target(self) -> Path:
        """展开 ~ 为 home 目录。"""
        return Path(self.target_path).expanduser()


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def registry_path() -> Path:
    """注册表文件路径：~/.ae-sdd/distributors.json。"""
    return Path.home() / ".ae-sdd" / REGISTRY_FILENAME


def _default_distributors() -> list[DistributorEntry]:
    """种子注册表：首次运行或注册表缺失时使用。"""
    entries = []
    for name, cfg in _KNOWN_AGENTS.items():
        entries.append(DistributorEntry(
            name=name,
            protocol=cfg["protocol"],
            target_path=cfg["target_path"],
            detect=cfg["detect"],
            detect_cli=cfg["detect_cli"],
            enabled=True,
            registered_at=_utc_now(),
            notes=cfg["notes"],
            l2_global_file=cfg.get("l2_global_file"),
            l2_language=cfg.get("l2_language"),
        ))
    return entries


def load_registry() -> list[DistributorEntry]:
    """加载注册表；文件不存在时用种子初始化并落盘。"""
    path = registry_path()
    if not path.is_file():
        entries = _default_distributors()
        save_registry(entries)
        return entries
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        # 损坏时回退种子（不覆盖损坏文件，让用户决定）
        return _default_distributors()
    raw = data.get("distributors") if isinstance(data, dict) else None
    if not isinstance(raw, list):
        return _default_distributors()
    entries = [DistributorEntry.from_dict(item) for item in raw]
    # 🆕 v3.10.8 迁移：旧注册表条目缺少 l2_global_file/l2_language 字段时，
    # 从 _KNOWN_AGENTS 回填（已知 agent 才回填；自定义 agent 保持 None）。
    migrated = False
    for e in entries:
        if e.l2_global_file is None and e.l2_language is None:
            cfg = _KNOWN_AGENTS.get(e.name)
            if cfg and cfg.get("l2_global_file"):
                e.l2_global_file = cfg["l2_global_file"]
                e.l2_language = cfg["l2_language"]
                migrated = True
    if migrated:
        save_registry(entries)
    return entries


def save_registry(entries: list[DistributorEntry]) -> None:
    """落盘注册表。"""
    path = registry_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "schema_version": SCHEMA_VERSION,
        "distributors": [e.to_dict() for e in entries],
    }
    path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


def find_entry(entries: list[DistributorEntry], name: str) -> Optional[DistributorEntry]:
    """按 name 查找条目。"""
    return next((e for e in entries if e.name == name), None)


def register_one(
    name: str,
    protocol: str,
    target_path: str,
    detect: str = "always",
    detect_cli: Optional[str] = None,
    enabled: bool = True,
    notes: str = "",
    force: bool = False,
) -> tuple[bool, str, list[DistributorEntry]]:
    """注册一个分发器。返回 (success, message, updated_entries)。

    - name 已存在且 force=False → 失败
    - protocol 非法 → 失败
    """
    if protocol not in ("copytree", "harness_mount"):
        return False, f"未知协议: {protocol}（可选 copytree / harness_mount）", []
    if detect not in ("always", "path_exists", "cli_exists"):
        return False, f"未知 detect: {detect}（可选 always / path_exists / cli_exists）", []
    if detect == "cli_exists" and not detect_cli:
        return False, "detect=cli_exists 时必须指定 --detect-cli", []

    entries = load_registry()
    existing = find_entry(entries, name)
    if existing and not force:
        return False, f"分发器 '{name}' 已存在（用 --force 覆盖）", entries

    entry = DistributorEntry(
        name=name,
        protocol=protocol,
        target_path=target_path,
        detect=detect,
        detect_cli=detect_cli,
        enabled=enabled,
        registered_at=_utc_now(),
        notes=notes,
    )
    if existing:
        idx = entries.index(existing)
        entries[idx] = entry
    else:
        entries.append(entry)
    save_registry(entries)
    return True, f"已注册分发器 '{name}'（protocol={protocol}, enabled={enabled}）", entries


def unregister_one(name: str) -> tuple[bool, str, list[DistributorEntry]]:
    """注销分发器：从注册表删除条目。返回 (success, message, updated_entries)。"""
    entries = load_registry()
    existing = find_entry(entries, name)
    if not existing:
        return False, f"分发器 '{name}' 不存在", entries
    entries.remove(existing)
    save_registry(entries)
    return True, f"已注销分发器 '{name}'", entries


def set_enabled(name: str, enabled: bool) -> tuple[bool, str, list[DistributorEntry]]:
    """启用/禁用分发器（软注销/恢复）。返回 (success, message, updated_entries)。"""
    entries = load_registry()
    existing = find_entry(entries, name)
    if not existing:
        return False, f"分发器 '{name}' 不存在", entries
    if existing.enabled == enabled:
        state = "启用" if enabled else "禁用"
        return True, f"分发器 '{name}' 已是{state}状态", entries
    existing.enabled = enabled
    save_registry(entries)
    state = "已启用" if enabled else "已禁用"
    return True, f"分发器 '{name}' {state}", entries


# ─── 扫描 ────────────────────────────────────────────────────────────────────

def _cli_exists(cmd: str) -> bool:
    """检查 CLI 是否在 PATH 中。"""
    return shutil.which(cmd) is not None


def evaluate_detect(entry: DistributorEntry) -> bool:
    """运行时 detect 判定：该 Agent 当前是否可用。

    - always: 永远 True（向后兼容 claude）
    - path_exists: target_path 存在
    - cli_exists: detect_cli 在 PATH
    """
    if entry.detect == "always":
        return True
    if entry.detect == "path_exists":
        return entry.resolved_target().exists()
    if entry.detect == "cli_exists":
        return _cli_exists(entry.detect_cli or "")
    return False


def scan_for_agents() -> list[dict[str, Any]]:
    """扫描 ~/.*/skills/ 目录，识别已安装 ae-sdd 副本的 Agent。

    返回建议注册清单：[{name, protocol, target_path, detect, detect_cli, notes, found}]
    found=True 表示扫描到该 Agent 的安装痕迹。
    """
    home = Path.home()
    found: list[dict[str, Any]] = []
    for name, cfg in _KNOWN_AGENTS.items():
        target = Path(cfg["target_path"]).expanduser()
        is_found = False
        if cfg["detect"] == "cli_exists":
            is_found = _cli_exists(cfg["detect_cli"] or "")
        elif cfg["detect"] == "path_exists":
            is_found = target.exists()
        else:  # always
            is_found = True
        if is_found:
            found.append({
                "name": name,
                "protocol": cfg["protocol"],
                "target_path": cfg["target_path"],
                "detect": cfg["detect"],
                "detect_cli": cfg["detect_cli"],
                "notes": cfg["notes"],
                "found": True,
            })
    return found


def scan_unregistered() -> list[dict[str, Any]]:
    """扫描已安装但未注册的 Agent，返回建议注册清单。"""
    entries = load_registry()
    registered_names = {e.name for e in entries}
    return [s for s in scan_for_agents() if s["name"] not in registered_names]
