#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR/frontend"

pnpm --filter nubase_cli typecheck
pnpm --filter nubase_cli build
pnpm --filter nubase_cli test
pnpm --filter nubase_cli pack:check

echo "Ready to publish:"
echo "  cd frontend/packages/mcp-bridge"
echo "  npm publish --access public"
