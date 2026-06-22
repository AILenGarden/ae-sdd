#!/usr/bin/env bash
# init.sh — ae-sdd 项目实例化（薄壳）
#
# 🆕 v3.0 P0 三件套之 init（2026-06-18）：
#   给具体项目（如 icec-cloud-boss）创建 .ae-sdd/ 骨架。
#
# 真正的实现见 scripts/init.py（跨平台，零外部依赖）。
#
# 用法: bash scripts/init.sh <project-dir> <project-key> [选项]

set -e

PYTHON="${PYTHON:-}"
if [[ -z "$PYTHON" ]]; then
  for cmd in "python" "python3"; do
    if command -v "$cmd" >/dev/null 2>&1; then
      if "$cmd" --version >/dev/null 2>&1; then
        PYTHON="$cmd"
        break
      fi
    fi
  done
  if [[ -z "$PYTHON" ]] && command -v py >/dev/null 2>&1; then
    PYTHON="py -3"
  fi
  if [[ -z "$PYTHON" ]]; then
    echo "❌ 致命：未找到 python / python3 / py，请先安装 Python 3.8+" >&2
    exit 1
  fi
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
exec "$PYTHON" "$SCRIPT_DIR/init.py" "$@"
