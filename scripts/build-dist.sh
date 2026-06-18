#!/usr/bin/env bash
# sync-to-plugin.sh — 把 ae-sdd 母版同步到两个目标：
#   1) 本地 Claude skills 安装目录（即时生效）
#   2) 仓库内 plugins/ae-sdd/      （marketplace plugin 副本）
#
# 🆕 v3.0 变更：母版根 SKILL.md 即为 ae-sdd 唯一主入口，
#               不再从 skills/orchestration/ae-sdd-skill.md 派生。
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
# 同步函数：把母版整树覆盖到 DST，剥离仓库管理产物
#
# 🆕 v3.0：母版根 SKILL.md 就是主入口，tar 复制时已经包含，
#          无需任何"刷新 SKILL.md"操作。
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

  # 🆕 v3.0 验证：确保主入口 SKILL.md 存在且非空
  if [[ ! -f "$DST/SKILL.md" ]] || [[ ! -s "$DST/SKILL.md" ]]; then
    echo "🔴 致命：$DST/SKILL.md 缺失或为空，主入口未同步成功" >&2
    exit 1
  fi
}

# --------------------------------------------------------------------------
# 执行同步
# --------------------------------------------------------------------------

# 母版根 SKILL.md 校验（v3.0 起的硬前置 — 主入口缺失则不同步）
if [[ ! -f "$SRC/SKILL.md" ]] || [[ ! -s "$SRC/SKILL.md" ]]; then
  echo "🔴 致命：母版根 SKILL.md 缺失或为空，请先修复主入口" >&2
  exit 1
fi

# 母版不再需要"刷新 SKILL.md"步骤 — 主入口就是根 SKILL.md。
# 历史遗留的"从 skills/orchestration/ae-sdd-skill.md 派生"逻辑已删除。

sync_to "$DST_LOCAL"
echo "✅ 本地 Claude skills 同步完成 → $DST_LOCAL"

sync_to "$DST_MARKET"
echo "✅ marketplace plugin 副本同步完成 → $DST_MARKET"
