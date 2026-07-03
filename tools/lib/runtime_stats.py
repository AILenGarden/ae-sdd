"""
Lightweight runtime statistics for ae-sdd commands.

The recorder is intentionally best-effort: statistics must never fail the
business command. Events are stored as JSONL under `.ae-sdd/runtime-stats/`
or an override directory from `AE_SDD_STATS_DIR`.
"""
from __future__ import annotations

import contextvars
import json
import os
import tempfile
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional


_CURRENT: contextvars.ContextVar["TraceRecorder | None"] = contextvars.ContextVar(
    "ae_sdd_runtime_stats_current",
    default=None,
)

_FALSE_VALUES = {"0", "false", "no", "off"}
_SENSITIVE_MARKERS = ("password", "passwd", "secret", "token", "apikey", "api-key", "key")

# 🆕 2026-07-03 缺口1:bootstrap import 成本计量。
# 入口脚本(tools/bin/ae-sdd)在所有 import 之前用 perf_counter_ns 打戳到此 env。
# start_command 读它算出 bootstrapMs(= start_command 时刻 − 进程启动),
# 与 durationMs(纯业务函数耗时)分离,让 doctor 能真实诊断 import 固定成本。
_BOOT_NS_ENV = "AE_SDD_BOOT_NS"


def is_enabled() -> bool:
    return os.environ.get("AE_SDD_STATS", "1").strip().lower() not in _FALSE_VALUES


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def _find_project_root(start: Optional[Path] = None) -> Optional[Path]:
    cur = (start or Path.cwd()).expanduser().resolve()
    if cur.name == ".ae-sdd":
        return cur.parent
    if (cur / ".ae-sdd").is_dir():
        return cur
    for parent in cur.parents:
        if (parent / ".ae-sdd").is_dir():
            return parent
    return None


def _detect_scale(project: Optional[Path] = None) -> Optional[str]:
    """🆕 2026-07-03(B3): 从项目 state.json 读取 scale（大/中/小/微），无则 None。

    用于 runtime_stats 按 scale 分桶，诊断"微任务 vs 大任务开销比例失调"。
    失败静默（统计不得阻断业务命令），返回 None。
    """
    try:
        root = _find_project_root(project)
        if root is None:
            return None
        state_path = root / ".ae-sdd" / "state.json"
        if not state_path.is_file():
            return None
        with state_path.open("r", encoding="utf-8") as fh:
            data = json.load(fh)
        scale = data.get("scale")
        return str(scale) if scale else None
    except Exception:
        return None


def stats_dir(project: Optional[Path] = None) -> Path:
    override = os.environ.get("AE_SDD_STATS_DIR", "").strip()
    if override:
        return Path(override).expanduser()

    root = _find_project_root(project)
    if root is not None:
        return root / ".ae-sdd" / "runtime-stats"

    return Path(tempfile.gettempdir()) / "ae-sdd" / "runtime-stats"


def _event_file(directory: Path, started_at: Optional[str] = None) -> Path:
    day = (started_at or _utc_now())[:10]
    return directory / f"{day}.jsonl"


def _is_sensitive_flag(value: str) -> bool:
    key = value.lstrip("-").split("=", 1)[0].lower()
    return any(marker in key for marker in _SENSITIVE_MARKERS)


def _redact_argv(argv: list[str]) -> list[str]:
    redacted: list[str] = []
    redact_next = False
    for item in argv:
        text = str(item)
        if redact_next:
            redacted.append("***")
            redact_next = False
            continue
        if "=" in text and _is_sensitive_flag(text.split("=", 1)[0]):
            redacted.append(f"{text.split('=', 1)[0]}=***")
            continue
        if text.startswith("-") and _is_sensitive_flag(text):
            redacted.append(text)
            redact_next = True
            continue
        redacted.append(text)
    return redacted


@dataclass
class TraceRecorder:
    command: str
    argv: list[str]
    directory: Path
    attrs: dict[str, Any] = field(default_factory=dict)
    started_at: str = field(default_factory=_utc_now)
    started_ns: int = field(default_factory=time.perf_counter_ns)
    started_cpu_ns: int = field(default_factory=time.process_time_ns)
    spans: list[dict[str, Any]] = field(default_factory=list)
    suppressed: bool = False
    # 🆕 2026-07-03 缺口1:CLI 顶层 import 固定成本(进程启动 → start_command)。
    # None 表示未打戳(旧入口或子进程未继承),to_event 时省略该字段以保向后兼容。
    bootstrap_ms: Optional[float] = None

    def to_event(self, exit_code: int, error_class: Optional[str] = None) -> dict[str, Any]:
        duration_ms = (time.perf_counter_ns() - self.started_ns) / 1_000_000
        cpu_ms = (time.process_time_ns() - self.started_cpu_ns) / 1_000_000
        event = {
            "schema": "ae-sdd.runtimeStats.v1",
            "startedAt": self.started_at,
            "finishedAt": _utc_now(),
            "command": self.command,
            "argv": _redact_argv(self.argv),
            "exitCode": int(exit_code),
            "durationMs": round(duration_ms, 3),
            "cpuMs": round(cpu_ms, 3),
            "spans": self.spans,
            "attrs": self.attrs,
        }
        # 🆕 2026-07-03(B3): scale 提升为顶层字段,便于 summarize 按 scale 分桶。
        scale = self.attrs.get("scale")
        if scale:
            event["scale"] = scale
        # 🆕 2026-07-03 缺口1:bootstrapMs 提升为顶层字段(与 scale 同级)。
        # 仅在入口打过戳时写入;旧事件/无戳子进程无此字段,summarize 用 .get() 容错。
        if self.bootstrap_ms is not None:
            event["bootstrapMs"] = round(self.bootstrap_ms, 3)
        if error_class:
            event["errorClass"] = error_class
        return event


class Span:
    def __init__(self, name: str, attrs: Optional[dict[str, Any]] = None) -> None:
        self.name = name
        self.attrs: dict[str, Any] = dict(attrs or {})
        self.duration_ms = 0.0
        self.cpu_ms = 0.0
        self._started_ns = 0
        self._started_cpu_ns = 0
        self._finished = False

    def __enter__(self) -> "Span":
        self._started_ns = time.perf_counter_ns()
        self._started_cpu_ns = time.process_time_ns()
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        if exc_type is not None:
            self.attrs.setdefault("errorClass", getattr(exc_type, "__name__", str(exc_type)))
        self.finish()

    def finish(self) -> None:
        if self._finished:
            return
        self._finished = True
        self.duration_ms = round((time.perf_counter_ns() - self._started_ns) / 1_000_000, 3)
        self.cpu_ms = round((time.process_time_ns() - self._started_cpu_ns) / 1_000_000, 3)
        recorder = _CURRENT.get()
        if recorder is None:
            return
        recorder.spans.append({
            "name": self.name,
            "durationMs": self.duration_ms,
            "cpuMs": self.cpu_ms,
            "attrs": _jsonable(self.attrs),
        })


def _jsonable(value: Any) -> Any:
    if isinstance(value, Path):
        return str(value)
    if isinstance(value, dict):
        return {str(k): _jsonable(v) for k, v in value.items()}
    if isinstance(value, (list, tuple, set)):
        return [_jsonable(v) for v in value]
    try:
        json.dumps(value)
        return value
    except TypeError:
        return str(value)


def start_command(
    command: str,
    argv: Optional[list[str]] = None,
    project: Optional[Path] = None,
    attrs: Optional[dict[str, Any]] = None,
) -> Optional[TraceRecorder]:
    if not is_enabled():
        _CURRENT.set(None)
        return None
    merged_attrs = _jsonable(attrs or {})
    # 🆕 2026-07-03(B3): 探测 scale 写入 attrs，事件落盘后供 summarize 按 scale 分桶。
    scale = _detect_scale(project)
    if scale:
        merged_attrs["scale"] = scale
    # 🆕 2026-07-03 缺口1:读入口 env 戳算 bootstrap import 成本。
    # 入口脚本在 import 前打戳;此处(业务函数执行前)与之相减即为 CLI 顶层 import 固定成本。
    # 失败静默(统计不得阻断业务);无戳时 bootstrap_ms=None,to_event 省略该字段。
    bootstrap_ms: Optional[float] = None
    boot_raw = os.environ.get(_BOOT_NS_ENV, "").strip()
    if boot_raw:
        try:
            bootstrap_ms = (time.perf_counter_ns() - int(boot_raw)) / 1_000_000
        except (ValueError, TypeError):
            bootstrap_ms = None
    recorder = TraceRecorder(
        command=command or "unknown",
        argv=list(argv or []),
        directory=stats_dir(project),
        attrs=merged_attrs,
        bootstrap_ms=bootstrap_ms,
    )
    _CURRENT.set(recorder)
    return recorder


def current() -> Optional[TraceRecorder]:
    return _CURRENT.get()


def suppress_current_event() -> None:
    recorder = _CURRENT.get()
    if recorder is not None:
        recorder.suppressed = True


def span(name: str, attrs: Optional[dict[str, Any]] = None) -> Span:
    return Span(name, attrs)


def finish_command(exit_code: int = 0, error_class: Optional[str] = None) -> int:
    recorder = _CURRENT.get()
    _CURRENT.set(None)
    if recorder is None or recorder.suppressed:
        return int(exit_code)

    try:
        recorder.directory.mkdir(parents=True, exist_ok=True)
        event = recorder.to_event(exit_code=exit_code, error_class=error_class)
        target = _event_file(recorder.directory, recorder.started_at)
        with target.open("a", encoding="utf-8", newline="\n") as fh:
            fh.write(json.dumps(event, ensure_ascii=False, sort_keys=True) + "\n")
    except Exception:
        pass
    # 🆕 2026-07-03 缺口1:清理入口 env 戳,防子进程(如 gates 调 scanner)继承到
    # 父进程的打戳时刻,导致子进程 bootstrapMs 算成"父进程启动→子进程 start"的大偏差。
    # 子进程入口会自行 setdefault 重新打戳;此处 pop 仅清当前进程环境,不影响已派生子进程。
    try:
        os.environ.pop(_BOOT_NS_ENV, None)
    except Exception:
        pass
    return int(exit_code)


def read_events(limit: int = 100, project: Optional[Path] = None) -> list[dict[str, Any]]:
    directory = stats_dir(project)
    if limit <= 0 or not directory.exists():
        return []

    events: list[dict[str, Any]] = []
    try:
        files = sorted(directory.glob("*.jsonl"), reverse=True)
        for file in files:
            try:
                lines = file.read_text(encoding="utf-8", errors="replace").splitlines()
            except OSError:
                continue
            for line in reversed(lines):
                if not line.strip():
                    continue
                try:
                    events.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
                if len(events) >= limit:
                    return events
    except Exception:
        return events
    return events


def clear_events(project: Optional[Path] = None) -> int:
    directory = stats_dir(project)
    if not directory.exists():
        return 0
    count = 0
    for file in directory.glob("*.jsonl"):
        try:
            file.unlink()
            count += 1
        except OSError:
            pass
    return count


def _percentile(values: list[float], pct: float) -> float:
    if not values:
        return 0.0
    ordered = sorted(values)
    index = int(round((len(ordered) - 1) * pct))
    return round(ordered[index], 3)


def summarize_events(events: list[dict[str, Any]], slow_limit: int = 10) -> dict[str, Any]:
    durations = [float(e.get("durationMs") or 0.0) for e in events]
    # 🆕 2026-07-03 缺口3:cpuMs 已采集但此前未汇总,无法区分"CPU 慢"与"等子进程 I/O 慢"。
    # ioWaitMs = duration − cpu(clamp≥0),衡量 I/O 等待(主要是子进程 subprocess)占比。
    cpus = [float(e.get("cpuMs") or 0.0) for e in events]
    io_waits = [max(0.0, d - c) for d, c in zip(durations, cpus)]
    bootstraps = [float(e.get("bootstrapMs") or 0.0) for e in events if e.get("bootstrapMs") is not None]
    command_map: dict[str, dict[str, Any]] = {}
    spans: list[dict[str, Any]] = []

    for event in events:
        command = str(event.get("command") or "unknown")
        duration = float(event.get("durationMs") or 0.0)
        cpu = float(event.get("cpuMs") or 0.0)
        item = command_map.setdefault(command, {
            "command": command,
            "count": 0,
            "totalMs": 0.0,
            "maxMs": 0.0,
            "totalCpuMs": 0.0,
            "maxCpuMs": 0.0,
            "lastStartedAt": "",
        })
        item["count"] += 1
        item["totalMs"] += duration
        item["maxMs"] = max(float(item["maxMs"]), duration)
        item["totalCpuMs"] += cpu
        item["maxCpuMs"] = max(float(item["maxCpuMs"]), cpu)
        if not item["lastStartedAt"]:
            item["lastStartedAt"] = event.get("startedAt", "")
        for span_event in event.get("spans", []) or []:
            span_copy = dict(span_event)
            span_copy["command"] = command
            span_copy["startedAt"] = event.get("startedAt", "")
            spans.append(span_copy)

    commands = []
    for item in command_map.values():
        total = float(item["totalMs"])
        count = int(item["count"])
        item["totalMs"] = round(total, 3)
        item["avgMs"] = round(total / count, 3) if count else 0.0
        item["maxMs"] = round(float(item["maxMs"]), 3)
        item["avgCpuMs"] = round(float(item["totalCpuMs"]) / count, 3) if count else 0.0
        item["maxCpuMs"] = round(float(item["maxCpuMs"]), 3)
        item["totalCpuMs"] = round(float(item["totalCpuMs"]), 3)
        commands.append(item)

    slowest_commands = sorted(
        events,
        key=lambda e: float(e.get("durationMs") or 0.0),
        reverse=True,
    )[:slow_limit]
    slowest_spans = sorted(
        spans,
        key=lambda e: float(e.get("durationMs") or 0.0),
        reverse=True,
    )[:slow_limit]

    # 🆕 2026-07-03(B3): 按 scale 分桶，诊断"微任务 vs 大任务开销比例失调"。
    # scale 缺失（旧事件或无 state.json）归入 "unknown"。
    by_scale: dict[str, dict[str, Any]] = {}
    for event in events:
        scale = str(event.get("scale") or event.get("attrs", {}).get("scale") or "unknown")
        bucket = by_scale.setdefault(scale, {
            "scale": scale,
            "count": 0,
            "totalMs": 0.0,
            "maxMs": 0.0,
            "totalCpuMs": 0.0,
        })
        duration = float(event.get("durationMs") or 0.0)
        cpu = float(event.get("cpuMs") or 0.0)
        bucket["count"] += 1
        bucket["totalMs"] += duration
        bucket["maxMs"] = max(float(bucket["maxMs"]), duration)
        bucket["totalCpuMs"] += cpu
    scale_list = []
    for bucket in by_scale.values():
        total = float(bucket["totalMs"])
        count = int(bucket["count"])
        bucket["totalMs"] = round(total, 3)
        bucket["avgMs"] = round(total / count, 3) if count else 0.0
        bucket["maxMs"] = round(float(bucket["maxMs"]), 3)
        # 🆕 2026-07-03 缺口3:byScale 补 cpu/ioWait,诊断某规模是否卡在 I/O 等待。
        bucket["avgCpuMs"] = round(float(bucket["totalCpuMs"]) / count, 3) if count else 0.0
        bucket["avgIoWaitMs"] = round(max(0.0, bucket["avgMs"] - bucket["avgCpuMs"]), 3)
        bucket["totalCpuMs"] = round(float(bucket["totalCpuMs"]), 3)
        scale_list.append(bucket)
    scale_list.sort(key=lambda b: float(b["avgMs"]), reverse=True)

    # 比例失调诊断：微任务平均开销 / 大任务平均开销。
    # 微任务走 8-phase 子链、大任务走 14-phase，理论上微任务应显著更轻；
    # 若比值接近或超过 1，说明微任务可能误判走大链或编码段未瘦身（B1 修复前的症状）。
    scale_ratios: dict[str, float] = {}
    avg_by_scale = {b["scale"]: float(b["avgMs"]) for b in scale_list}
    big_avg = avg_by_scale.get("大")
    if big_avg and big_avg > 0:
        for s in ("微", "小", "中"):
            v = avg_by_scale.get(s)
            if v and v > 0:
                scale_ratios[f"{s}/大"] = round(v / big_avg, 3)

    return {
        "count": len(events),
        "duration": {
            "avgMs": round(sum(durations) / len(durations), 3) if durations else 0.0,
            "p50Ms": _percentile(durations, 0.50),
            "p95Ms": _percentile(durations, 0.95),
            "maxMs": round(max(durations), 3) if durations else 0.0,
        },
        # 🆕 2026-07-03 缺口3:cpuMs/ioWaitMs 分桶,让 doctor 能区分 CPU 瓶颈 vs I/O 等待。
        "cpuMs": {
            "avgMs": round(sum(cpus) / len(cpus), 3) if cpus else 0.0,
            "p50Ms": _percentile(cpus, 0.50),
            "p95Ms": _percentile(cpus, 0.95),
            "maxMs": round(max(cpus), 3) if cpus else 0.0,
        },
        "ioWaitMs": {
            "avgMs": round(sum(io_waits) / len(io_waits), 3) if io_waits else 0.0,
            "p50Ms": _percentile(io_waits, 0.50),
            "p95Ms": _percentile(io_waits, 0.95),
            "maxMs": round(max(io_waits), 3) if io_waits else 0.0,
        },
        # 🆕 2026-07-03 缺口1:bootstrapMs 分桶(仅含打过戳的事件),供 doctor 真实诊断 import 固定成本。
        "bootstrapMs": {
            "count": len(bootstraps),
            "avgMs": round(sum(bootstraps) / len(bootstraps), 3) if bootstraps else 0.0,
            "p50Ms": _percentile(bootstraps, 0.50),
            "p95Ms": _percentile(bootstraps, 0.95),
            "maxMs": round(max(bootstraps), 3) if bootstraps else 0.0,
        } if bootstraps else {"count": 0, "avgMs": 0.0, "p50Ms": 0.0, "p95Ms": 0.0, "maxMs": 0.0},
        "commands": sorted(commands, key=lambda e: float(e["totalMs"]), reverse=True),
        "slowestCommands": [
            {
                "command": e.get("command"),
                "durationMs": e.get("durationMs"),
                "cpuMs": e.get("cpuMs"),
                "bootstrapMs": e.get("bootstrapMs"),
                "exitCode": e.get("exitCode"),
                "startedAt": e.get("startedAt"),
                "scale": e.get("scale") or (e.get("attrs") or {}).get("scale"),
            }
            for e in slowest_commands
        ],
        "slowestSpans": slowest_spans,
        "byScale": scale_list,
        "scaleRatios": scale_ratios,
    }
