#!/usr/bin/env bash
# dev-sync.sh — ae-sdd 开发者工具（薄壳）
#
# 🆕 v3.0.1 跨平台化（2026-06-18）：
#   旧：此文件包含完整 sync 逻辑（依赖 bash + fswatch 等）
#   新：薄壳，仅做"找 Python + exec dev_sync.py"
#
# 真正的实现见 scripts/dev_sync.py（跨平台，零外部依赖）。
#
# 用法: bash scripts/dev-sync.sh [--build-only | --install-only | --watch | --uninstall]

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
exec "$PYTHON" "$SCRIPT_DIR/dev_sync.py" "$@"
