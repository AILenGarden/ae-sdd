"""
Subprocess wrapper for ae-sdd runtime statistics.

It keeps the normal subprocess.run surface while forcing UTF-8 text decoding on
Windows and recording command spans when runtime_stats is active.
"""
from __future__ import annotations

import os
import subprocess
from pathlib import Path
from typing import Any, Optional, Sequence

from lib import runtime_stats


def _command_label(args: Sequence[Any] | str) -> str:
    if isinstance(args, str):
        return args.split()[0] if args.strip() else "subprocess"
    if args:
        return Path(str(args[0])).name
    return "subprocess"


def run_command(
    args: Sequence[Any] | str,
    *,
    span_name: Optional[str] = None,
    cwd: Optional[str | Path] = None,
    env: Optional[dict[str, str]] = None,
    timeout: Optional[float] = None,
    check: bool = False,
    capture_output: bool = True,
    text: bool = True,
    encoding: Optional[str] = "utf-8",
    errors: Optional[str] = "replace",
    **kwargs: Any,
) -> subprocess.CompletedProcess:
    merged_env = os.environ.copy()
    merged_env.setdefault("PYTHONUTF8", "1")
    merged_env.setdefault("PYTHONIOENCODING", "utf-8")
    if env:
        merged_env.update(env)

    label = _command_label(args)
    attrs = {
        "cmd": label,
        "cwd": str(cwd) if cwd is not None else "",
        "timeoutSec": timeout,
    }
    with runtime_stats.span(span_name or f"subprocess:{label}", attrs) as sp:
        try:
            run_kwargs: dict[str, Any] = {
                "cwd": cwd,
                "env": merged_env,
                "timeout": timeout,
                "check": check,
                "capture_output": capture_output,
                "text": text,
            }
            if text:
                run_kwargs["encoding"] = encoding
                run_kwargs["errors"] = errors
            run_kwargs.update(kwargs)
            result = subprocess.run(args, **run_kwargs)
            sp.attrs["exitCode"] = result.returncode
            if isinstance(result.stdout, str):
                sp.attrs["stdoutChars"] = len(result.stdout)
            if isinstance(result.stderr, str):
                sp.attrs["stderrChars"] = len(result.stderr)
            return result
        except subprocess.TimeoutExpired as exc:
            sp.attrs["timeout"] = True
            sp.attrs["timeoutSec"] = timeout
            if isinstance(exc.stdout, str):
                sp.attrs["stdoutChars"] = len(exc.stdout)
            if isinstance(exc.stderr, str):
                sp.attrs["stderrChars"] = len(exc.stderr)
            raise
