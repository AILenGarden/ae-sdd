#!/usr/bin/env bash
# Preflight checks before running the UT coverage loop.
#
# Purpose: separate "environment is broken" from "coverage too low" so the
# loop never burns iterations on a project that can't build in the first
# place. Especially important inside corporate networks where missing
# settings.xml / private repos / proxy / JDK mismatch are the real blockers.
#
# Usage:
#   preflight.sh <project-root> [--module <module-relpath>] [--skip <name>]...
#
# Skippable check names: effective_settings, effective_pom,
# dependency_resolve, test_compile
#
# Output (stdout, JSON):
#   {
#     "ok": true|false,
#     "mvn_cmd": "./mvnw" | "mvn",
#     "mvn_version": "3.9.6",
#     "java_version": "17.0.10",
#     "pom_dir": "/abs/path",
#     "checks": [
#       {"name": "mvn_available", "ok": true,  "detail": "..."},
#       ...
#     ],
#     "first_failure": "dependency_resolve" | null,
#     "hint": "..." | null
#   }
#
# Exit codes:
#   0 — all checks passed
#   1 — at least one check failed (see JSON)
#   2 — usage error

set -u

if [[ $# -lt 1 ]]; then
  echo "usage: $0 <project-root> [--module <module>] [--skip <name>]..." >&2
  exit 2
fi

PROJECT_ROOT="$1"; shift
MODULE=""
SKIP_LIST=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --module) MODULE="$2"; shift 2 ;;
    --skip)   SKIP_LIST="$SKIP_LIST,$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

[[ -d "$PROJECT_ROOT" ]] || { echo "project root not a directory: $PROJECT_ROOT" >&2; exit 2; }

POM_DIR="$PROJECT_ROOT"
[[ -n "$MODULE" ]] && POM_DIR="$PROJECT_ROOT/$MODULE"
[[ -f "$POM_DIR/pom.xml" ]] || { echo "no pom.xml at: $POM_DIR" >&2; exit 2; }

skipped() { [[ ",$SKIP_LIST," == *",$1,"* ]]; }

# Pick build command: project mvnw wins.
MVN_CMD=""
if [[ -x "$PROJECT_ROOT/mvnw" ]]; then
  MVN_CMD="$PROJECT_ROOT/mvnw"
elif command -v mvn >/dev/null 2>&1; then
  MVN_CMD="mvn"
fi

# Result accumulators. Each entry encoded as: name<TAB>ok<TAB>detail
# (TAB-separated, with newlines escaped in detail).
RESULTS_FILE=$(mktemp)
trap 'rm -f "$RESULTS_FILE"' EXIT

FIRST_FAIL=""
HINT=""
MVN_VERSION=""
JAVA_VERSION=""

record() {
  local name="$1" ok="$2" detail="$3"
  # Replace TAB and newline in detail so we can use TSV.
  detail=$(printf '%s' "$detail" | tr '\t\n' '  ')
  printf '%s\t%s\t%s\n' "$name" "$ok" "$detail" >> "$RESULTS_FILE"
  if [[ "$ok" != "true" && -z "$FIRST_FAIL" ]]; then
    FIRST_FAIL="$name"
  fi
}

run_mvn() {
  ( cd "$POM_DIR" && "$MVN_CMD" -B "$@" 2>&1 )
}

# ---- check: mvn_available ----
if [[ -z "$MVN_CMD" ]]; then
  record "mvn_available" false "neither ./mvnw nor mvn found on PATH"
  HINT="Install Maven or use a project with mvnw wrapper."
else
  V=$("$MVN_CMD" -v 2>&1 | head -5 || true)
  MVN_VERSION=$(printf '%s' "$V" | grep -oE 'Apache Maven [0-9.]+' | awk '{print $3}' | head -1)
  JAVA_VERSION=$(printf '%s' "$V" | grep -oE 'Java version: [0-9._]+' | awk '{print $3}' | head -1)
  record "mvn_available" true "$MVN_CMD ($MVN_VERSION, java $JAVA_VERSION)"
fi

if [[ -n "$MVN_CMD" ]]; then

  # ---- check: effective_settings ----
  if ! skipped "effective_settings"; then
    OUT=$(run_mvn help:effective-settings -q 2>&1 || true)
    if printf '%s' "$OUT" | grep -qE 'BUILD FAILURE|\[ERROR\]'; then
      FIRST=$(printf '%s' "$OUT" | grep -E '\[ERROR\]' | head -3 | paste -sd' | ' -)
      record "effective_settings" false "${FIRST:-unknown error}"
      [[ -z "$HINT" ]] && HINT="settings.xml unreadable — check ~/.m2/settings.xml and any company-specific config."
    else
      record "effective_settings" true "ok"
    fi
  fi

  # ---- check: effective_pom (parent reachable) ----
  if ! skipped "effective_pom"; then
    OUT=$(run_mvn help:effective-pom -q 2>&1 || true)
    if printf '%s' "$OUT" | grep -qE 'Non-resolvable parent POM|Could not find artifact|BUILD FAILURE'; then
      FIRST=$(printf '%s' "$OUT" | grep -E '\[ERROR\]|Non-resolvable' | head -3 | paste -sd' | ' -)
      record "effective_pom" false "${FIRST:-unknown error}"
      [[ -z "$HINT" ]] && HINT="Parent POM not resolvable — confirm the internal repository / mirror is configured."
    else
      record "effective_pom" true "ok"
    fi
  fi

  # ---- check: dependency_resolve (offline first, then online) ----
  if ! skipped "dependency_resolve"; then
    OUT=$(run_mvn dependency:resolve -q -o 2>&1 || true)
    if printf '%s' "$OUT" | grep -qE 'BUILD FAILURE'; then
      OUT=$(run_mvn dependency:resolve -q 2>&1 || true)
    fi
    if printf '%s' "$OUT" | grep -qE 'Could not resolve|Could not find artifact|BUILD FAILURE'; then
      FIRST=$(printf '%s' "$OUT" | grep -E 'Could not resolve|Could not find artifact|\[ERROR\]' | head -3 | paste -sd' | ' -)
      record "dependency_resolve" false "${FIRST:-unknown error}"
      [[ -z "$HINT" ]] && HINT="Dependency resolution failed — likely missing credentials or unreachable internal repo. Show this error to your build infra owner."
    else
      record "dependency_resolve" true "ok"
    fi
  fi

  # ---- check: test_compile (only if no earlier failure) ----
  if ! skipped "test_compile" && [[ -z "$FIRST_FAIL" ]]; then
    OUT=$(run_mvn test-compile -q 2>&1 || true)
    if printf '%s' "$OUT" | grep -qE 'BUILD FAILURE|COMPILATION ERROR'; then
      FIRST=$(printf '%s' "$OUT" | grep -E '\[ERROR\]' | head -5 | paste -sd' | ' -)
      record "test_compile" false "${FIRST:-unknown error}"
      [[ -z "$HINT" ]] && HINT="Test sources don't compile yet — fix compile errors first, then re-run the loop."
    else
      record "test_compile" true "ok"
    fi
  fi
fi

# ---- emit JSON ----
OK="true"
[[ -n "$FIRST_FAIL" ]] && OK="false"

MVN_CMD="$MVN_CMD" MVN_VERSION="$MVN_VERSION" JAVA_VERSION="$JAVA_VERSION" \
POM_DIR="$POM_DIR" RESULTS_FILE="$RESULTS_FILE" \
FIRST_FAIL="$FIRST_FAIL" HINT="$HINT" OK="$OK" \
python3 <<'PY'
import json, os

def opt(v):
    return v if v else None

checks = []
with open(os.environ["RESULTS_FILE"], "r", encoding="utf-8") as f:
    for line in f:
        line = line.rstrip("\n")
        if not line:
            continue
        parts = line.split("\t", 2)
        if len(parts) != 3:
            continue
        name, ok, detail = parts
        checks.append({"name": name, "ok": ok == "true", "detail": detail})

out = {
    "ok": os.environ["OK"] == "true",
    "mvn_cmd": opt(os.environ["MVN_CMD"]),
    "mvn_version": opt(os.environ["MVN_VERSION"]),
    "java_version": opt(os.environ["JAVA_VERSION"]),
    "pom_dir": os.environ["POM_DIR"],
    "checks": checks,
    "first_failure": opt(os.environ["FIRST_FAIL"]),
    "hint": opt(os.environ["HINT"]),
}
print(json.dumps(out, indent=2, ensure_ascii=False))
PY

[[ "$OK" == "true" ]] && exit 0 || exit 1
