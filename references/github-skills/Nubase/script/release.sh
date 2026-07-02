#!/usr/bin/env bash
# One-stop release entry point for Nubase.
#
# Two independent release units:
#   cli      - the nubase_cli MCP bridge, published to npm (frontend/packages/mcp-bridge).
#              Consumers pick it up via `npx -y nubase_cli@latest`.
#   backend  - the all-in-one jar (API + bundled Studio UI), built with the Maven
#              `with-frontend` profile and rolling-deployed to the fleet. Studio is NOT
#              a separate deploy unit — it is compiled into the jar, so any Studio change
#              ships by redeploying the backend.
#
# Usage:
#   script/release.sh cli                  # publish nubase_cli to npm
#   script/release.sh backend [args...]    # build + rolling-deploy the jar
#   script/release.sh all [args...]        # cli first, then backend
#
# Extra args after `backend`/`all` are passed through to rolling-deploy.sh, e.g.:
#   script/release.sh backend --no-build   # deploy the existing target/ jar
#   script/release.sh backend --only 1     # deploy a single server (0-based index)
#
# npm 2FA: `npm publish` prompts for your authenticator OTP in an interactive terminal.
set -euo pipefail

cd "$(cd "$(dirname "$0")/.." && pwd)"   # repo root

CLI_DIR="frontend/packages/mcp-bridge"
ROLLING_DEPLOY="script/deploy/rolling-deploy.sh"

log()  { echo -e "\n>>> $*"; }
die()  { echo "❌ $*" >&2; exit 1; }

confirm() {
    # confirm "question" — returns 0 on yes. Auto-yes when not a TTY.
    [ -t 0 ] || return 0
    read -r -p "$1 [y/N] " ans
    [[ "$ans" =~ ^[Yy]$ ]]
}

release_cli() {
    command -v npm >/dev/null || die "npm not found on PATH."
    local version
    version=$(node -p "require('./${CLI_DIR}/package.json').version")
    local published
    published=$(npm view nubase_cli version 2>/dev/null || echo "none")

    log "nubase_cli: local=${version}, npm latest=${published}"
    [ "${version}" != "${published}" ] || die "Version ${version} is already published. Bump the version in ${CLI_DIR}/package.json and src/index.ts (CLI_VERSION) first."

    confirm "Publish nubase_cli@${version} to npm?" || { echo "Skipped cli publish."; return 0; }

    log "Building + publishing nubase_cli@${version} (npm will prompt for your OTP)..."
    ( cd "${CLI_DIR}" && npm run build && npm_config_cache=../../.npm-cache npm publish )
    log "✅ Published nubase_cli@${version}"
}

release_backend() {
    [ -x "${ROLLING_DEPLOY}" ] || die "${ROLLING_DEPLOY} not found or not executable."
    log "Building + rolling-deploying the all-in-one jar (API + bundled Studio)..."
    "${ROLLING_DEPLOY}" "$@"
}

target="${1:-}"
[ -n "${target}" ] || die "Usage: script/release.sh {cli|backend|all} [rolling-deploy args...]"
shift || true

case "${target}" in
    cli)     release_cli ;;
    backend) release_backend "$@" ;;
    all)     release_cli; release_backend "$@" ;;
    *)       die "Unknown target '${target}'. Use: cli | backend | all" ;;
esac

log "✅ Release complete: ${target}"
