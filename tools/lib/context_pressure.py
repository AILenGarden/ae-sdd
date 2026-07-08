"""
context_pressure.py — ae-sdd 节点级上下文压力软提示（🆕 v3.5.5）

定位：节点级"事前预警"，与 v3.3.0 PRD 级 compact（"事后收尾"）互补不替代。
对应 SOP：source/SKILL.md §⏱️ 节点级上下文压力软提示

输入信号源（全为 state.json / session.json 已有字段，无新 schema 必填）：
  - session.userConfirmedPhases.length  已确认审核点数
  - state.events.length                 流程操作次数
  - state.history.length                phase 跳转次数
  - .ae-sdd/ + .auto-engineering/ 落盘文档总字节
  - state.activeAgents.length           当前并发 sub-agent 数

可配置 override：config.yaml 第 4 维 contextPressure.thresholds.<medium|high|critical>.<signal>
缺省静态表见 DEFAULT_THRESHOLDS。

设计约束（🔴 红线）：
  - 仅软提示（report-only），不阻断流程
  - 不自动 compact，不自动派 sub-agent
  - medium/high：输出数据 + 评级
  - critical：额外输出推荐动作清单
"""
from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional


# ─── 缺省阈值表（v3.5.5 首版硬编码，可被 config.yaml override）────────────────
DEFAULT_THRESHOLDS: dict[str, dict[str, int]] = {
    "medium":   {"docBytes": 500_000,   "events": 100, "historyLen": 5,  "confirmedPhases": 3, "activeAgents": 2},
    "high":     {"docBytes": 2_000_000, "events": 200, "historyLen": 8,  "confirmedPhases": 4, "activeAgents": 3},
    "critical": {"docBytes": 5_000_000, "events": 400, "historyLen": 10, "confirmedPhases": 5, "activeAgents": 4},
}


# 评级顺序（从低到高），OR 触发取最高档
_LEVELS: tuple[str, ...] = ("low", "medium", "high", "critical")

# critical 时输出的推荐动作清单（不阻断，仅参考）
_CRITICAL_SUGGESTIONS: tuple[str, ...] = (
    "运行 ae-sdd state prd-check-complete 看是否可进入 PRD 收尾",
    "考虑 PRD 收尾 + ae-sdd runtime compact",
    "考虑拆分当前 Story 为多个独立 Story",
)

# signal 名 → 评级档位（critical 是 high 的子集，独立判定）
_SIGNAL_NAMES: tuple[str, ...] = (
    "docBytes", "events", "historyLen", "confirmedPhases", "activeAgents",
)


@dataclass
class PressureSignal:
    """单个信号当前值与触发的阈值档位"""
    signal: str
    value: int
    threshold: str  # "low" | "medium" | "high" | "critical"
    limit: int      # 触发该档位的阈值


@dataclass
class PressureReport:
    """上下文压力报告（report-only，不阻断）"""
    pressure: str = "low"                          # 总体评级
    signals: dict[str, int] = field(default_factory=dict)   # 5 信号当前值
    triggered: list[PressureSignal] = field(default_factory=list)  # 触发的档位
    thresholds: dict[str, dict[str, int]] = field(default_factory=dict)  # 实际生效的阈值
    suggestions: list[str] = field(default_factory=list)     # critical 时填充

    def to_dict(self) -> dict:
        return {
            "pressure": self.pressure,
            "signals": dict(self.signals),
            "triggeredSignals": [
                {"signal": t.signal, "value": t.value, "threshold": t.threshold, "limit": t.limit}
                for t in self.triggered
            ],
            "thresholds": {k: dict(v) for k, v in self.thresholds.items()},
            "suggestions": list(self.suggestions),
            "nextAction": "context-pressure is informational only; no action required",
        }


def _scan_doc_bytes(base_dirs: list[Path]) -> int:
    """扫描 base_dirs 下所有文件总字节数（缺省 0）。"""
    total = 0
    for base in base_dirs:
        if not base.is_dir():
            continue
        for p in base.rglob("*"):
            if p.is_file():
                try:
                    total += p.stat().st_size
                except OSError:
                    pass
    return total


def _safe_load_json(path: Path) -> Optional[dict]:
    """加载 JSON 文件；不存在/损坏/IO 错误返回 None。"""
    if not path.is_file():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None


def _parse_nested_config(config_path: Path) -> dict:
    """轻量 YAML 解析（仅 contextPressure 用途，支持任意嵌套深度）。

    paths.read_config 只支持 1 层缩进，无法读 thresholds.<level>.<signal>。
    此处独立实现，约定：
      - 顶层 key 不带缩进
      - 子级用 2 空格缩进（每层 +2）
      - 末梢 value 形如 "key: value" 或 "key: number"（数字自动 int/float）
    """
    out: dict = {}
    if not config_path.is_file():
        return out
    text = config_path.read_text(encoding="utf-8")

    # 缩进宽度检测：取最小缩进作为单位（约定为 2 空格，但容忍 4）
    stack: list[tuple[int, dict]] = [(-1, out)]
    for raw in text.splitlines():
        line = raw.split("#", 1)[0].rstrip()
        if not line.strip():
            continue
        # 计算缩进空格数
        stripped = line.lstrip(" ")
        indent = len(line) - len(stripped)
        # 弹出栈直到 indent 合适
        while stack and stack[-1][0] >= indent:
            stack.pop()
        parent = stack[-1][1]
        if ":" in stripped:
            key, _, val = stripped.partition(":")
            key = key.strip()
            val = val.strip()
            if not val:
                # 新建子 dict
                new_dict: dict = {}
                parent[key] = new_dict
                stack.append((indent, new_dict))
            else:
                # 末梢 value，尝试转数字
                try:
                    if "." in val:
                        parent[key] = float(val)
                    else:
                        parent[key] = int(val)
                except ValueError:
                    val = val.strip('"').strip("'")
                    parent[key] = val
    return out


def _resolve_thresholds(ade_sdd: Optional[Path]) -> dict[str, dict[str, int]]:
    """合并缺省阈值与 config.yaml override。config 缺字段回退到缺省。

    读取路径：.ae-sdd/config.yaml 的 contextPressure.thresholds.<level>.<signal>。
    任一字段缺失或非法 → 保留缺省值，不抛异常。
    """
    merged: dict[str, dict[str, int]] = {
        level: dict(v) for level, v in DEFAULT_THRESHOLDS.items()
    }
    if ade_sdd is None:
        return merged
    cfg = _parse_nested_config(ade_sdd / "config.yaml")
    cp_block = cfg.get("contextPressure") if isinstance(cfg, dict) else None
    if not isinstance(cp_block, dict):
        return merged
    th = cp_block.get("thresholds")
    if not isinstance(th, dict):
        return merged
    for level in ("medium", "high", "critical"):
        override = th.get(level)
        if not isinstance(override, dict):
            continue
        for sig in _SIGNAL_NAMES:
            v = override.get(sig)
            if isinstance(v, (int, float)) and v > 0:
                merged[level][sig] = int(v)
    return merged


def _rate_signal(value: int, thresholds: dict[str, dict[str, int]]) -> tuple[str, int]:
    """单个信号评级：从 low 往上匹配第一个满足的档位；返回 (档位, 阈值)。

    判定逻辑：value >= threshold[level][signal] → 该档位触发。
    取最高触发的档位；都不触发 → low。
    """
    fired = "low"
    fired_limit = 0
    for level in _LEVELS[1:]:  # medium / high / critical
        limit = thresholds[level]
        # 任一信号字段存在且超过 → 升级
        # 单一 signal 调用时只比一个字段，这里我们用传入 signal 名
        # 但 _rate_signal 不知道具体 signal 名，由调用方传字典
        # 此处只取 level 中该 signal 的限制
        # 用 _rate_signal_with_name 更精确
        pass
    return fired, fired_limit


def _evaluate_signals(signals: dict[str, int], thresholds: dict[str, dict[str, int]]) -> tuple[str, list[PressureSignal]]:
    """5 个信号统一评级 + 收集触发项。OR 触发取最高档。

    返回 (overall_pressure, triggered_signals)。
    """
    overall_idx = 0  # low
    triggered: list[PressureSignal] = []
    for sig in _SIGNAL_NAMES:
        val = int(signals.get(sig, 0))
        for level in ("medium", "high", "critical"):
            limit = thresholds[level].get(sig, 0)
            if val >= limit > 0:
                # 触发了这个档位
                level_idx = _LEVELS.index(level)
                if level_idx > overall_idx:
                    overall_idx = level_idx
                triggered.append(PressureSignal(signal=sig, value=val, threshold=level, limit=limit))
    return _LEVELS[overall_idx], triggered


def run_all(ade_sdd: Optional[Path], story_id: str = "") -> PressureReport:
    """主入口：采集 5 信号 + 应用阈值 → 返回 PressureReport。

    Args:
        ade_sdd: 项目 .ae-sdd 目录（None 表示无项目上下文）
        story_id: Story ID（空 = 项目级）；决定 session.json / state.json 路径
    """
    thresholds = _resolve_thresholds(ade_sdd)
    signals: dict[str, int] = {
        "docBytes": 0,
        "events": 0,
        "historyLen": 0,
        "confirmedPhases": 0,
        "activeAgents": 0,
    }

    # 1. session.json → userConfirmedPhases.length
    if ade_sdd is not None:
        from lib import session as session_mod
        sess = session_mod.read_session(ade_sdd, story_id)
        if isinstance(sess, dict):
            signals["confirmedPhases"] = len(sess.get("userConfirmedPhases") or [])

    # 2. state.json → events.length + history.length + activeAgents.length
    if ade_sdd is not None:
        from lib import state as state_mod
        from lib import paths as paths_mod
        # state.json 路径：项目级为 .ae-sdd/state.json；Story 级兼容 R6 与 legacy work-item 目录
        if story_id:
            sp = (
                paths_mod.find_work_item_state_path(ade_sdd, story_id)
                or paths_mod.project_root(ade_sdd) / ".auto-engineering" / story_id / "state.json"
            )
        else:
            sp = paths_mod.state_path(ade_sdd)
        st = state_mod.read_state(sp)
        if isinstance(st, dict):
            signals["events"] = len(st.get("events") or [])
            signals["historyLen"] = len(st.get("history") or [])
            signals["activeAgents"] = len(st.get("activeAgents") or [])

    # 3. 落盘文档总字节
    if ade_sdd is not None:
        from lib import paths as paths_mod
        proj_root = paths_mod.project_root(ade_sdd)
        bases: list[Path] = []
        if story_id:
            state_file = paths_mod.find_work_item_state_path(ade_sdd, story_id)
            if state_file is not None:
                bases.append(state_file.parent)
            bases.append(proj_root / ".auto-engineering" / story_id)
            bases.append(proj_root / "design")
            bases.append(proj_root / "task")
        bases.append(ade_sdd)
        signals["docBytes"] = _scan_doc_bytes(bases)

    # 4. 评级
    pressure, triggered = _evaluate_signals(signals, thresholds)

    # 5. critical 时填充 suggestions
    suggestions: list[str] = []
    if pressure == "critical":
        suggestions = list(_CRITICAL_SUGGESTIONS)

    return PressureReport(
        pressure=pressure,
        signals=signals,
        triggered=triggered,
        thresholds=thresholds,
        suggestions=suggestions,
    )
