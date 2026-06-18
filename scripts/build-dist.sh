#!/usr/bin/env bash
# build-dist.sh — ae-sdd 母版构建（薄壳）
#
# 🆕 v3.0.1 跨平台化（2026-06-18）：
#   旧：此文件包含完整 build 逻辑（依赖 tar/date 等 Unix 工具）
#   新：薄壳，仅做"找 Python + exec build_dist.py"
#
# 真正的实现见 scripts/build_dist.py（跨平台，零外部依赖）。
#
# 用法: bash scripts/build-dist.sh

set -e

# 定位 Python（兼容 Windows Git Bash / macOS / Linux）
# 顺序：python（Windows 真 Python 走这条） → python3（macOS/Linux 标准） → py -3（Windows launcher）
PYTHON="${PYTHON:-}"
if [[ -z "$PYTHON" ]]; then
  for cmd in "python" "python3"; do
    if command -v "$cmd" >/dev/null 2>&1; then
      # 验证是真 Python 不是 WindowsApps stub（stub 跑 --version 没输出）
      if "$cmd" --version >/dev/null 2>&1; then
        PYTHON="$cmd"
        break
      fi
    fi
  done
  # 最后回退 py -3（Windows launcher）
  if [[ -z "$PYTHON" ]] && command -v py >/dev/null 2>&1; then
    PYTHON="py -3"
  fi
  if [[ -z "$PYTHON" ]]; then
    echo "❌ 致命：未找到 python / python3 / py，请先安装 Python 3.8+" >&2
    exit 1
  fi
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
exec "$PYTHON" "$SCRIPT_DIR/build_dist.py" "$@"
