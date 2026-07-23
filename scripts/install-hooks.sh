#!/usr/bin/env bash
# install-hooks.sh — 首次安装 / 重新安装 ae-sdd 母版的 git hooks
#
# 作用：
#   - 设置 git core.hooksPath = .githooks
#   - chmod +x .githooks/post-commit
#   - 验证 .githooks/post-commit 存在
#
# 用法：
#   bash scripts/install-hooks.sh
#   bash scripts/install-hooks.sh --uninstall    # 恢复 git 默认 hooksPath

set -eu

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || echo "")"
if [ -z "$REPO_ROOT" ]; then
    echo "❌ 当前目录不是 git 仓库"
    exit 1
fi
cd "$REPO_ROOT"

if [ "${1:-}" = "--uninstall" ]; then
    git config --unset core.hooksPath || true
    echo "✅ 已卸载 hooks（恢复 git 默认 .git/hooks）"
    exit 0
fi

if [ ! -f .githooks/post-commit ]; then
    echo "❌ .githooks/post-commit 不存在（请确认仓库完整）"
    exit 1
fi

chmod +x .githooks/post-commit
git config core.hooksPath .githooks

echo "✅ ae-sdd git hooks 已安装："
echo "   hooksPath = .githooks"
echo "   post-commit: $(ls -la .githooks/post-commit | awk '{print $1, $NF}')"
echo ""
echo "下次 git commit 时将自动触发分发闭环："
echo "   build_dist → install → harness adapter → harness remount"
echo ""
echo "旁路开关: SKIP_AE_SDD_HOOK=1 git commit -m '...'"
echo "卸载:     bash scripts/install-hooks.sh --uninstall"