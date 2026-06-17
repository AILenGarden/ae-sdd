#!/usr/bin/env bash
# install.sh — ae-sdd SKILL 安装脚本
#
# 支持两种模式：
#   远程模式（curl | bash）：自动 git clone 后安装
#   本地模式（bash scripts/install.sh）：需在仓库根目录执行
#
# 安装目标：$HOME/.claude/skills/ae-sdd

set -euo pipefail

REPO_URL="https://github.com/AILenGarden/ae-sdd.git"
SKILL_NAME="ae-sdd"
DST="$HOME/.claude/skills/$SKILL_NAME"

# ─── 颜色输出 ───────────────────────────────────────────────────────────────
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

info()    { echo -e "${GREEN}[ae-sdd]${NC} $*"; }
warn()    { echo -e "${YELLOW}[ae-sdd] ⚠${NC}  $*"; }
error()   { echo -e "${RED}[ae-sdd] ✗${NC}  $*" >&2; }
success() { echo -e "${GREEN}[ae-sdd] ✅${NC} $*"; }

# ─── 检测运行模式 ─────────────────────────────────────────────────────────────
TMPDIR_CREATED=""
SRC=""

detect_mode() {
  # 优先判断：当前工作目录是仓库根（bash scripts/install.sh 的常见场景）
  # 其次判断：脚本所在目录的父目录是仓库根（bash /abs/path/scripts/install.sh 场景）
  local SCRIPT_PARENT
  SCRIPT_PARENT="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." 2>/dev/null && pwd)" || SCRIPT_PARENT=""

  if [[ -f "$(pwd)/plugins/ae-sdd/SKILL.md" ]]; then
    SRC="$(pwd)"
    info "检测到本地仓库模式，使用 $SRC"
  elif [[ -n "$SCRIPT_PARENT" && -f "$SCRIPT_PARENT/plugins/ae-sdd/SKILL.md" ]]; then
    SRC="$SCRIPT_PARENT"
    info "检测到本地仓库模式，使用 $SRC"
  else
    # 不在仓库根：尝试 git clone
    if ! command -v git &>/dev/null; then
      error "未找到 git，请先安装 git 后重试，或手动 clone 仓库后执行 bash scripts/install.sh"
      exit 1
    fi
    TMPDIR_CREATED="$(mktemp -d)"
    info "远程模式：正在 clone 仓库..."
    git clone --depth=1 "$REPO_URL" "$TMPDIR_CREATED/ae-sdd" 2>&1 | sed "s/^/  /"
    SRC="$TMPDIR_CREATED/ae-sdd"
    info "Clone 完成"
  fi
}

# ─── 备份旧版本 ───────────────────────────────────────────────────────────────
backup_existing() {
  if [[ -d "$DST" ]]; then
    local BAK="${DST}.bak.$(date +%Y%m%d%H%M%S)"
    warn "检测到已有安装版本，备份到："
    warn "  $BAK"
    mv "$DST" "$BAK"
  fi
}

# ─── 复制文件 ─────────────────────────────────────────────────────────────────
install_files() {
  local PLUGIN_SRC="$SRC/plugins/ae-sdd"
  if [[ ! -d "$PLUGIN_SRC" ]]; then
    error "未找到 $PLUGIN_SRC，仓库结构异常"
    cleanup
    exit 1
  fi
  mkdir -p "$DST"
  cp -r "$PLUGIN_SRC/." "$DST/"
  info "文件已复制到 $DST"
}

# ─── 验证安装 ─────────────────────────────────────────────────────────────────
verify() {
  if [[ ! -f "$DST/SKILL.md" ]]; then
    error "安装验证失败：$DST/SKILL.md 不存在"
    cleanup
    exit 1
  fi
}

# ─── 清理临时目录 ─────────────────────────────────────────────────────────────
cleanup() {
  if [[ -n "$TMPDIR_CREATED" && -d "$TMPDIR_CREATED" ]]; then
    rm -rf "$TMPDIR_CREATED"
  fi
}

# ─── 打印使用提示 ─────────────────────────────────────────────────────────────
print_usage() {
  echo ""
  success "ae-sdd SKILL 安装成功！"
  echo ""
  echo "  安装路径：$DST"
  echo ""
  echo "  在 Claude Code 中使用："
  echo "    输入  /ae-sdd  启动自动化工程助手"
  echo ""
  echo "  更多信息：https://github.com/AILenGarden/ae-sdd"
  echo ""
}

# ─── 主流程 ───────────────────────────────────────────────────────────────────
main() {
  echo ""
  info "开始安装 ae-sdd SKILL..."
  echo ""

  detect_mode
  backup_existing
  install_files
  verify
  cleanup
  print_usage
}

main "$@"
