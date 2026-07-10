#!/usr/bin/env bash
# ae-sdd.sh — ae-sdd CLI 薄壳入口
#
# 🆕 v3.0 P0 三件套（2026-06-18）：
#   把 ae-sdd CLI 跨平台暴露，让用户能直接 `ae-sdd <cmd>` 而不用每次写 python 路径。
#
# 真正的实现见 tools/bin/ae-sdd（跨平台，零外部依赖）。
#
# 用法: bash scripts/ae-sdd.sh <command> [...]
# 或先把 tools/bin/ 加到 PATH 直接跑 ae-sdd

set -e

# PYTHON 探测结果存数组，避免 "py -3" 这类含空格的回退值被 "$PYTHON" 当成单个命令名（见 A2 修复）。
PYTHON_CMD=()
if [[ -n "${PYTHON:-}" ]]; then
  # 用户通过环境变量传入时，按空白拆成数组（兼容 PYTHON="py -3"）。
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

# 找 tools/bin/ae-sdd（脚本所在仓库根/tools/bin/）
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
AE_SDD_BIN="$REPO_ROOT/tools/bin/ae-sdd"

if [[ ! -f "$AE_SDD_BIN" ]]; then
  echo "❌ 致命：未找到 $AE_SDD_BIN" >&2
  exit 1
fi

exec "${PYTHON_CMD[@]}" "$AE_SDD_BIN" "$@"
