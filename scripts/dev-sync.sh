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

# PYTHON 探测结果存数组，避免 "py -3" 这类含空格的回退值被 "$PYTHON" 当成单个命令名（见 A2 修复）。
PYTHON_CMD=()
if [[ -n "${PYTHON:-}" ]]; then
  read -r -a PYTHON_CMD <<< "$PYTHON"
fi
if [[ ${#PYTHON_CMD[@]} -eq 0 ]]; then
  for cmd in "python" "python3"; do
    if command -v "$cmd" >/dev/null 2>&1; then
      if "$cmd" --version >/dev/null 2>&1; then
        PYTHON_CMD=("$cmd")
        break
      fi
    fi
  done
  if [[ ${#PYTHON_CMD[@]} -eq 0 ]] && command -v py >/dev/null 2>&1; then
    PYTHON_CMD=(py -3)
  fi
  if [[ ${#PYTHON_CMD[@]} -eq 0 ]]; then
    echo "❌ 致命：未找到 python / python3 / py，请先安装 Python 3.8+" >&2
    exit 1
  fi
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
exec "${PYTHON_CMD[@]}" "$SCRIPT_DIR/dev_sync.py" "$@"
