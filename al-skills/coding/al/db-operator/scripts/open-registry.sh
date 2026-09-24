#!/usr/bin/env sh
# Local registration UI launcher for macOS and Linux. The registry server must run
# as root: every admin call is re-entered as the service identity so the registry,
# vault, and capability records keep their owner. The server only listens on
# 127.0.0.1 and refuses non-loopback Host/Origin headers.
set -eu

no_browser=0
if [ "${1:-}" = "--no-browser" ]; then
  no_browser=1
fi

skill_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
server="$skill_dir/scripts/registry-server.mjs"

case "$(uname -s)" in
  Darwin|Linux) ;;
  *) echo "Unsupported operating system: $(uname -s)" >&2; exit 1 ;;
esac

# Runs as root after re-entry, so open the page as the invoking user when possible.
open_browser() {
  if [ "$(uname -s)" = Darwin ]; then
    if [ -n "${SUDO_USER:-}" ]; then
      sudo -u "$SUDO_USER" open "$1" 2>/dev/null || true
    else
      open "$1" 2>/dev/null || true
    fi
  else
    if [ -n "${SUDO_USER:-}" ]; then
      sudo -u "$SUDO_USER" xdg-open "$1" >/dev/null 2>&1 || true
    else
      xdg-open "$1" >/dev/null 2>&1 || true
    fi
  fi
}

if ! command -v node >/dev/null 2>&1; then
  echo 'Node.js is required to run the registration UI.' >&2
  exit 1
fi

port=${DB_OPERATOR_UI_PORT:-17842}
ui_url="http://127.0.0.1:$port/"

# An already running registration UI is reused instead of started twice; the
# non-root branch runs this check before asking for sudo.
session=$(curl -fsS --max-time 2 "$ui_url"api/session 2>/dev/null || true)
if [ -n "$session" ] && printf '%s' "$session" | grep -q '"service":"db-operator"'; then
  if [ "$no_browser" -eq 0 ]; then
    open_browser "$ui_url"
  fi
  exit 0
fi
if [ -n "$session" ]; then
  echo "Port $port is occupied by a different service; set DB_OPERATOR_UI_PORT to change it." >&2
  exit 1
fi

if [ "$(id -u)" -ne 0 ]; then
  echo 'Administrator privileges are required to start the registration server.'
  echo "Re-running with sudo; enter the administrator password when prompted."
  exec sudo -p "Password for %p: " "$0" "$@"
fi

log_file=$(mktemp -t db-operator-ui)
DB_OPERATOR_HOME=${DB_OPERATOR_HOME:-/var/lib/db-operator} \
  node "$server" >"$log_file" 2>&1 &
server_pid=$!

attempt=0
while [ "$attempt" -lt 40 ]; do
  if ! kill -0 "$server_pid" 2>/dev/null; then
    echo 'Registration server exited before becoming ready.' >&2
    sed 's/^/  /' "$log_file" >&2
    rm -f "$log_file"
    exit 1
  fi
  ready=$(curl -fsS --max-time 1 "$ui_url"api/session 2>/dev/null || true)
  if [ -n "$ready" ] && printf '%s' "$ready" | grep -q '"service":"db-operator"'; then
    if [ "$no_browser" -eq 0 ]; then
      open_browser "$ui_url"
    fi
    echo "Registration UI: $ui_url"
    exit 0
  fi
  attempt=$((attempt + 1))
  sleep 0.25 2>/dev/null || sleep 1
done

echo 'Registration server did not become ready.' >&2
sed 's/^/  /' "$log_file" >&2
exit 1
