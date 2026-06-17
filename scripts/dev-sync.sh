#!/usr/bin/env bash
# dev-sync.sh — ae-sdd 母版仓库开发者用的同步脚本
#
# 在仓库根目录运行,把当前母版同步到:
#   1) 本地 Claude skills 安装目录（直接生效,dev 调试用）
#   2) 仓库内 plugins/ae-sdd/ （marketplace plugin 副本）
#   3) marketplace 注册（写到 ~/.claude/plugins/known_marketplaces.json）
#
# 用法:
#   bash scripts/dev-sync.sh                   # 全部同步
#   bash scripts/dev-sync.sh --no-marketplace   # 跳过 marketplace 注册
#   bash scripts/dev-sync.sh --no-local         # 跳过本地 skills 同步
#   bash scripts/dev-sync.sh --no-plugin        # 跳过 marketplace 副本
#   bash scripts/dev-sync.sh --watch            # 监听模式: 文件变更自动同步

set -euo pipefail

SRC="$(cd "$(dirname "$0")/.." && pwd)"
SKILL_NAME="ae-sdd"
MARKET_KEY="ae-sdd-marketplace"
REPO_OWNER="AILenGarden"
REPO_NAME="ae-sdd"

# Windows 兼容: 优先用 USERPROFILE，再回退 HOME
HOME_DIR="${USERPROFILE:-$HOME}"
DST_LOCAL="$HOME_DIR/.claude/skills/${SKILL_NAME}"
DST_MARKET="$SRC/plugins/ae-sdd"
KNOWN_FILE="$HOME_DIR/.claude/plugins/known_marketplaces.json"

# 颜色
if [[ -t 1 ]]; then
  C_RED='\033[0;31m'; C_GREEN='\033[0;32m'; C_YELLOW='\033[0;33m'; C_BLUE='\033[0;34m'; C_RESET='\033[0m'
else
  C_RED=''; C_GREEN=''; C_YELLOW=''; C_BLUE=''; C_RESET=''
fi
info()  { printf "${C_BLUE}ℹ️  %s${C_RESET}\n" "$*"; }
ok()    { printf "${C_GREEN}✅ %s${C_RESET}\n" "$*"; }
warn()  { printf "${C_YELLOW}⚠️  %s${C_RESET}\n" "$*"; }
err()   { printf "${C_RED}❌ %s${C_RESET}\n" "$*" >&2; }
step()  { printf "\n${C_BLUE}== %s ==${C_RESET}\n" "$*"; }

# ---------------------------------------------------------------------------
# 参数
# ---------------------------------------------------------------------------
DO_LOCAL=1
DO_PLUGIN=1
DO_MARKET=1
WATCH=0

usage() {
  cat <<EOF
用法: bash scripts/dev-sync.sh [选项]

选项:
  --no-local       跳过本地 skills 同步
  --no-plugin      跳过仓库内 plugins/ae-sdd/ 副本同步
  --no-marketplace 跳过 marketplace 注册
  --watch          监听文件变更自动同步
  -h, --help       显示此帮助
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-local)       DO_LOCAL=0; shift ;;
    --no-plugin)      DO_PLUGIN=0; shift ;;
    --no-marketplace) DO_MARKET=0; shift ;;
    --watch)          WATCH=1; shift ;;
    -h|--help)        usage; exit 0 ;;
    *) err "未知参数: $1"; usage; exit 1 ;;
  esac
done

# ---------------------------------------------------------------------------
# 同步函数
# ---------------------------------------------------------------------------
sync_tree() {
  local DST="$1"
  local LABEL="$2"

  rm -rf "$DST"
  mkdir -p "$DST"

  # tar 管道复制,排除递归源
  ( cd "$SRC" && tar \
      --exclude='./plugins' \
      --exclude='./.git' \
      -cf - . \
  ) | ( cd "$DST" && tar -xf - )

  # 剥离 marketplace 注册表
  rm -f "$DST/.claude-plugin/marketplace.json"

  # 修复主入口 SKILL.md
  if [[ -f "$DST/skills/orchestration/ae-sdd-skill.md" ]]; then
    cp "$DST/skills/orchestration/ae-sdd-skill.md" "$DST/SKILL.md"
  fi

  ok "$LABEL 同步完成 → $DST"
}

sync_marketplace() {
  if ! command -v jq >/dev/null 2>&1; then
    warn "未找到 jq,跳过 marketplace 注册"
    return 0
  fi

  mkdir -p "$(dirname "$KNOWN_FILE")"
  local now; now="$(date -u +%Y-%m-%dT%H:%M:%S.000Z)"

  if [[ -f "$KNOWN_FILE" ]] && jq -e ".\"${MARKET_KEY}\"" "$KNOWN_FILE" >/dev/null 2>&1; then
    local tmp; tmp="$(mktemp)"
    jq --arg k "$MARKET_KEY" --arg ts "$now" \
      '.[$k].lastUpdated = $ts' "$KNOWN_FILE" > "$tmp" && mv "$tmp" "$KNOWN_FILE"
    rm -f "$tmp"
    info "marketplace '$MARKET_KEY' 已存在,刷新 lastUpdated"
  else
    if [[ -f "$KNOWN_FILE" ]]; then
      local tmp; tmp="$(mktemp)"
      jq --arg k "$MARKET_KEY" \
         --arg owner "$REPO_OWNER" --arg repo "$REPO_NAME" \
         --arg ts "$now" \
         '.[$k] = {"source": {"source": "github", "repo": ($owner + "/" + $repo)}, "lastUpdated": $ts}' \
         "$KNOWN_FILE" > "$tmp" && mv "$tmp" "$KNOWN_FILE"
      rm -f "$tmp"
    else
      cat > "$KNOWN_FILE" <<EOF
{
  "${MARKET_KEY}": {
    "source": {
      "source": "github",
      "repo": "${REPO_OWNER}/${REPO_NAME}"
    },
    "lastUpdated": "${now}"
  }
}
EOF
    fi
  fi
  ok "marketplace 注册刷新: $KNOWN_FILE"
}

# ---------------------------------------------------------------------------
# 单次同步
# ---------------------------------------------------------------------------
do_sync() {
  info "母版: $SRC"

  # 母版根 SKILL.md 也由本脚本刷新
  if [[ -f "$SRC/skills/orchestration/ae-sdd-skill.md" ]]; then
    cp "$SRC/skills/orchestration/ae-sdd-skill.md" "$SRC/SKILL.md"
    ok "母版根 SKILL.md 已刷新"
  fi

  if [[ $DO_LOCAL -eq 1 ]]; then
    step "同步到本地 skills"
    sync_tree "$DST_LOCAL" "本地 skills"
  fi

  if [[ $DO_PLUGIN -eq 1 ]]; then
    step "同步到 marketplace 副本"
    sync_tree "$DST_MARKET" "marketplace 副本"
  fi

  if [[ $DO_MARKET -eq 1 ]]; then
    step "刷新 marketplace 注册"
    sync_marketplace
  fi
}

# ---------------------------------------------------------------------------
# 监听模式
# ---------------------------------------------------------------------------
if [[ $WATCH -eq 1 ]]; then
  info "监听模式: 关注 skills/ standards/ templates/ assets/ 变化"
  if ! command -v fswatch >/dev/null 2>&1; then
    err "需要 fswatch: brew install fswatch (macOS) / apt install fswatch (Linux)"
    exit 1
  fi
  do_sync
  fswatch -r \
    -e "$SRC/plugins" \
    -e "$SRC/.git" \
    "$SRC/skills" "$SRC/standards" "$SRC/templates" "$SRC/assets" "$SRC/.claude-plugin" \
    | while read -r _; do
      do_sync
    done
else
  do_sync
fi
