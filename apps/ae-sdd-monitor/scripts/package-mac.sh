#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "macOS packaging must run on macOS so Electron can build and verify .app/.dmg artifacts." >&2
  exit 1
fi

npm run dist:mac
