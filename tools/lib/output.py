"""
output.py — ae-sdd CLI 输出工具

约定：
- 正常输出走 stdout，pipeline 友好
- 日志/进度走 stderr
- --json 选项：所有输出走 JSON 格式
"""
from __future__ import annotations

import json
import sys
from typing import Any


def emit(data: Any, *, as_json: bool = False) -> None:
    """输出数据。as_json=True 时输出 JSON 到 stdout。"""
    if as_json:
        json.dump(data, sys.stdout, ensure_ascii=False, indent=2, default=str)
        sys.stdout.write("\n")
    else:
        if isinstance(data, (dict, list)):
            json.dump(data, sys.stdout, ensure_ascii=False, indent=2, default=str)
            sys.stdout.write("\n")
        else:
            print(data)


def log(msg: str) -> None:
    """进度日志 → stderr"""
    print(msg, file=sys.stderr)


def err(msg: str) -> None:
    """错误 → stderr"""
    print(f"❌ {msg}", file=sys.stderr)


def ok(msg: str) -> None:
    """成功消息 → stderr（不污染 stdout 数据流）"""
    print(f"✅ {msg}", file=sys.stderr)


def warn(msg: str) -> None:
    """警告 → stderr"""
    print(f"⚠  {msg}", file=sys.stderr)


def info(msg: str) -> None:
    """信息 → stderr"""
    print(f"ℹ️  {msg}", file=sys.stderr)
