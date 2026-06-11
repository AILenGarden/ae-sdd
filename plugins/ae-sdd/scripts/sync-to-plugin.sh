#!/usr/bin/env bash
# sync-to-plugin.sh — 把 ae-sdd 母版同步到两个目标：
#   1) 本地 Claude skills 安装目录（即时生效）
#   2) 仓库内 plugins/ae-sdd/      （marketplace plugin 副本）
#
# 用法: bash scripts/sync-to-plugin.sh

set -e

SRC="$(cd "$(dirname "$0")/.." && pwd)"

# 目标 1: 本地 Claude skills 安装目录
DST_LOCAL="$USERPROFILE/.claude/skills/ae-sdd/skills/ae-sdd"
DST_LOCAL="${DST_LOCAL//\\//}"   # Windows 路径兼容

# 目标 2: marketplace plugin 副本（同仓库内，被 .claude-plugin/marketplace.json 引用）
DST_MARKET="$SRC/plugins/ae-sdd"

# --------------------------------------------------------------------------
# 同步函数：把母版整树覆盖到 DST，剥离仓库管理产物，并保证 SKILL.md 是最新入口
#
# 用 tar 管道复制并排除 plugins/、.git/，避免 DST 位于 SRC 子目录时（如 plugins/ae-sdd/）
# 出现 cp 递归错误（"cannot copy a directory into itself"）。
# --------------------------------------------------------------------------
sync_to() {
  local DST="$1"

  rm -rf "$DST"
  mkdir -p "$DST"

  # tar 管道复制，排除递归源和仓库私有数据
  ( cd "$SRC" && tar \
      --exclude='./plugins' \
      --exclude='./.git' \
      -cf - . \
  ) | ( cd "$DST" && tar -xf - )

  # 剥离副本内不该出现的项
  rm -f "$DST/.claude-plugin/marketplace.json"       # plugin 副本不持有 marketplace 注册表
  # 保留 $DST/.claude-plugin/plugin.json（plugin 自描述元数据，必须随副本一起分发）

  # 保证主入口 SKILL.md 始终是最新的 ae-sdd-skill.md（修复历史路径 bug）
  local SKILL_SRC="$DST/skills/orchestration/ae-sdd-skill.md"
  if [[ -f "$SKILL_SRC" ]]; then
    cp "$SKILL_SRC" "$DST/SKILL.md"
  else
    echo "⚠️  未找到 $SKILL_SRC，跳过 SKILL.md 刷新（请检查母版结构）" >&2
  fi
}

# --------------------------------------------------------------------------
# 执行同步
# --------------------------------------------------------------------------

# 母版根 SKILL.md 也由本脚本刷新（避免有人手改母版根 SKILL.md 而源不变，下次跑脚本被默默覆盖）
MASTER_SKILL_SRC="$SRC/skills/orchestration/ae-sdd-skill.md"
if [[ -f "$MASTER_SKILL_SRC" ]]; then
  cp "$MASTER_SKILL_SRC" "$SRC/SKILL.md"
  echo "✅ 母版根 SKILL.md 已刷新 → $SRC/SKILL.md"
else
  echo "⚠️  未找到 $MASTER_SKILL_SRC，母版根 SKILL.md 未刷新" >&2
fi

sync_to "$DST_LOCAL"
echo "✅ 本地 Claude skills 同步完成 → $DST_LOCAL"

sync_to "$DST_MARKET"
echo "✅ marketplace plugin 副本同步完成 → $DST_MARKET"
