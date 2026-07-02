#!/usr/bin/env bash
# Run a single Java test class with Maven + JaCoCo and print the report path.
#
# Usage:
#   run_coverage.sh <project-root> <TestClassFQN> [--module <module-relpath>]
#
# Examples:
#   run_coverage.sh ~/work/myrepo com.example.OrderServiceTest
#   run_coverage.sh ~/work/myrepo com.example.OrderServiceTest --module billing-svc
#
# Output (stderr): mvn build log
# Output (stdout): single line — absolute path of jacoco.xml on success.
# Exit:
#   0  -> tests passed AND jacoco.xml exists
#   1  -> tests failed
#   2  -> usage / pom error
#   3  -> tests passed but jacoco.xml not found (likely jacoco plugin missing)

set -u

if [[ $# -lt 2 ]]; then
  echo "usage: $0 <project-root> <TestClassFQN> [--module <module>]" >&2
  exit 2
fi

PROJECT_ROOT="$1"
TEST_FQN="$2"
shift 2

MODULE=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --module) MODULE="$2"; shift 2 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

if [[ ! -d "$PROJECT_ROOT" ]]; then
  echo "project root not a directory: $PROJECT_ROOT" >&2
  exit 2
fi

POM_DIR="$PROJECT_ROOT"
if [[ -n "$MODULE" ]]; then
  POM_DIR="$PROJECT_ROOT/$MODULE"
fi

if [[ ! -f "$POM_DIR/pom.xml" ]]; then
  echo "no pom.xml at: $POM_DIR" >&2
  exit 2
fi

# -fae = fail at end (still produces jacoco.xml even on test failures)
# jacoco:report binds to the verify phase; we invoke it explicitly so it runs
# even if a downstream module has issues unrelated to our target.
MVN_ARGS=(
  -B
  -fae
  -Dtest="$TEST_FQN"
  -DfailIfNoTests=false
  test
  jacoco:report
)

set +e
( cd "$POM_DIR" && mvn "${MVN_ARGS[@]}" ) 1>&2
MVN_EXIT=$?
set -e

# Locate jacoco.xml. Maven default: target/site/jacoco/jacoco.xml
REPORT="$POM_DIR/target/site/jacoco/jacoco.xml"
if [[ ! -f "$REPORT" ]]; then
  # Some projects override the path; do a single fallback scan.
  ALT=$(find "$POM_DIR/target" -maxdepth 4 -name 'jacoco.xml' -print -quit 2>/dev/null || true)
  if [[ -n "$ALT" && -f "$ALT" ]]; then
    REPORT="$ALT"
  else
    if [[ "$MVN_EXIT" -eq 0 ]]; then
      echo "tests passed but jacoco.xml not found under $POM_DIR/target — is the jacoco-maven-plugin configured?" >&2
      exit 3
    fi
    echo "$MVN_EXIT"  # placeholder; nothing to print
    exit 1
  fi
fi

# Always print the report path so callers can parse coverage even when tests
# failed (failed tests still yield a partial coverage report).
echo "$REPORT"
exit "$MVN_EXIT"
