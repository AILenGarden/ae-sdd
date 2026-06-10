#!/usr/bin/env bash
# sync-to-plugin.sh — 把 auto-engineering 同步到 skills-dir plugin
# 用法: bash scripts/sync-to-plugin.sh

SRC="$(cd "$(dirname "$0")/.." && pwd)"
DST="$USERPROFILE/.claude/skills/ae-sdd/skills/ae-sdd"
DST="${DST//\\//}"   # Windows 路径兼容

set -e

rm -rf "$DST"
mkdir -p "$DST"
cp -r "$SRC/." "$DST/"

# 保证主入口 SKILL.md 始终是最新的 ae-sdd-skill.md
cp "$DST/ae-sdd-skill.md" "$DST/SKILL.md"

echo "✅ 同步完成 → $DST"
