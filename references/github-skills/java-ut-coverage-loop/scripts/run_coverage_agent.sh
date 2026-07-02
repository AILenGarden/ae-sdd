#!/usr/bin/env bash
# Run a single Java test class with JaCoCo coverage WITHOUT requiring the
# project's pom.xml to declare the jacoco-maven-plugin.
#
# Strategy: attach jacocoagent.jar to the surefire JVM via -DargLine, then
# render the report locally with jacococli. The project's pom is not touched.
#
# Usage:
#   run_coverage_agent.sh <project-root> <TestClassFQN> [--module <module>] \
#       [--include <pkg.*>] [--jacoco-version <ver>]
#
# Output (stderr): mvn + jacococli log
# Output (stdout): single line — absolute path of jacoco.xml on success
# Exit:
#   0 — tests passed AND jacoco.xml produced
#   1 — tests failed (jacoco.xml may still be produced and path printed)
#   2 — usage / pom / environment error
#   4 — could not obtain jacocoagent.jar or jacococli.jar

set -u

if [[ $# -lt 2 ]]; then
  echo "usage: $0 <project-root> <TestClassFQN> [--module <module>] [--include <pkg.*>] [--jacoco-version <ver>]" >&2
  exit 2
fi

PROJECT_ROOT="$1"
TEST_FQN="$2"
shift 2

MODULE=""
INCLUDE=""
JACOCO_VER="0.8.11"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --module)          MODULE="$2"; shift 2 ;;
    --include)         INCLUDE="$2"; shift 2 ;;
    --jacoco-version)  JACOCO_VER="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

[[ -d "$PROJECT_ROOT" ]] || { echo "project root not a directory: $PROJECT_ROOT" >&2; exit 2; }

POM_DIR="$PROJECT_ROOT"
[[ -n "$MODULE" ]] && POM_DIR="$PROJECT_ROOT/$MODULE"
[[ -f "$POM_DIR/pom.xml" ]] || { echo "no pom.xml at: $POM_DIR" >&2; exit 2; }

# Pick mvn command: project mvnw wins.
MVN_CMD=""
if [[ -x "$PROJECT_ROOT/mvnw" ]]; then
  MVN_CMD="$PROJECT_ROOT/mvnw"
elif command -v mvn >/dev/null 2>&1; then
  MVN_CMD="mvn"
else
  echo "no mvn or mvnw found" >&2
  exit 2
fi

# Derive include from TEST_FQN if not given (drop the trailing class, then *).
if [[ -z "$INCLUDE" ]]; then
  PKG_PREFIX="${TEST_FQN%.*}"
  INCLUDE="${PKG_PREFIX}.*"
fi

M2="${HOME}/.m2/repository"

# ----- locate jacocoagent runtime jar -----
locate_or_fetch() {
  # $1 groupPath, $2 artifact, $3 version, $4 classifier (or empty)
  local gpath="$1" artifact="$2" ver="$3" classifier="$4"
  local jar dir
  dir="$M2/$gpath/$artifact/$ver"
  if [[ -n "$classifier" ]]; then
    jar="$dir/${artifact}-${ver}-${classifier}.jar"
  else
    jar="$dir/${artifact}-${ver}.jar"
  fi
  if [[ -f "$jar" ]]; then
    echo "$jar"
    return 0
  fi
  # Try fetching via mvn dependency:get (uses project's effective settings).
  local coords="org.jacoco:${artifact}:${ver}"
  [[ -n "$classifier" ]] && coords="${coords}:jar:${classifier}"
  ( cd "$POM_DIR" && "$MVN_CMD" -B dependency:get -Dartifact="$coords" -Dtransitive=false ) >&2 || return 1
  [[ -f "$jar" ]] && { echo "$jar"; return 0; } || return 1
}

JACOCO_AGENT=$(locate_or_fetch "org/jacoco" "org.jacoco.agent" "$JACOCO_VER" "runtime") || {
  echo "could not locate or fetch jacocoagent.jar (org.jacoco:org.jacoco.agent:$JACOCO_VER:jar:runtime)" >&2
  exit 4
}
JACOCO_CLI=$(locate_or_fetch "org/jacoco" "org.jacoco.cli" "$JACOCO_VER" "nodeps") || {
  echo "could not locate or fetch jacococli.jar (org.jacoco:org.jacoco.cli:$JACOCO_VER:jar:nodeps)" >&2
  exit 4
}

EXEC_FILE="$POM_DIR/target/jacoco.exec"
REPORT_DIR="$POM_DIR/target/jacoco-agent-report"
REPORT="$REPORT_DIR/jacoco.xml"
mkdir -p "$REPORT_DIR"
rm -f "$EXEC_FILE"

# ----- run tests with the agent attached -----
# Note: -DargLine overrides surefire's argLine. If the pom relies on argLine
# for something else (e.g. memory tuning), that gets lost. We accept this for
# the no-pom-change tradeoff; if users need to preserve it, they can switch to
# the plugin-based run_coverage.sh.
ARG_LINE="-javaagent:${JACOCO_AGENT}=destfile=${EXEC_FILE},append=false,includes=${INCLUDE}"

MVN_ARGS=(
  -B
  -fae
  -Dtest="$TEST_FQN"
  -DfailIfNoTests=false
  -DargLine="$ARG_LINE"
  test
)

set +e
( cd "$POM_DIR" && "$MVN_CMD" "${MVN_ARGS[@]}" ) 1>&2
MVN_EXIT=$?
set -e

if [[ ! -f "$EXEC_FILE" ]]; then
  echo "jacoco.exec not produced — agent injection may have been overridden by the pom's argLine. Check surefire config." >&2
  exit 2
fi

# ----- build the report -----
# Collect class + source roots for this module (best-effort, single module).
CLASS_DIR="$POM_DIR/target/classes"
SRC_DIR="$POM_DIR/src/main/java"

[[ -d "$CLASS_DIR" ]] || { echo "no compiled classes at $CLASS_DIR" >&2; exit 2; }

java -jar "$JACOCO_CLI" report "$EXEC_FILE" \
  --classfiles "$CLASS_DIR" \
  --sourcefiles "$SRC_DIR" \
  --xml "$REPORT" \
  --html "$REPORT_DIR/html" \
  1>&2 || {
    echo "jacococli failed to generate report" >&2
    exit 2
  }

[[ -f "$REPORT" ]] || { echo "report not produced at $REPORT" >&2; exit 2; }

echo "$REPORT"
exit "$MVN_EXIT"
