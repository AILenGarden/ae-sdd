#!/usr/bin/env sh
set -eu

if [ "$(id -u)" -ne 0 ]; then
  echo 'Run install-service.sh as root.' >&2
  exit 1
fi

agent_user=
connection=
agent_config=
register=0
build_from_source=0
while [ "$#" -gt 0 ]; do
  case "$1" in
    --agent-user) agent_user=$2; shift 2 ;;
    --connection) connection=$2; shift 2 ;;
    --agent-config) agent_config=$2; shift 2 ;;
    --register) register=1; shift ;;
    --build-from-source) build_from_source=1; shift ;;
    *) echo "Unknown option: $1" >&2; exit 2 ;;
  esac
done
[ -n "$agent_user" ] && [ -n "$connection" ] && [ -n "$agent_config" ] || {
  echo 'Required: --agent-user USER --connection ALIAS --agent-config PATH' >&2
  exit 2
}

skill_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
case "$(uname -s)" in
  Linux) platform=linux; service_user=db-operator; endpoint=/run/db-operator/db-operator.sock ;;
  Darwin) platform=macos; service_user=_dboperator; endpoint=/var/run/db-operator/db-operator.sock ;;
  *) echo 'Only Linux and macOS are supported.' >&2; exit 1 ;;
esac
case "$(uname -m)" in
  x86_64|amd64) architecture=x86_64 ;;
  arm64|aarch64) architecture=aarch64 ;;
  *) echo "Unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

agent_group=db-operator-agents
data_dir=/var/lib/db-operator
runtime_dir=$(dirname "$endpoint")
install_dir=/usr/local/libexec/db-operator
service_bin="$skill_dir/bin/service/$platform-$architecture"
admin_source="$service_bin/db-operator-admin"
daemon_source="$service_bin/db-operator-daemon"

if [ "$build_from_source" -eq 1 ] || [ ! -f "$admin_source" ] || [ ! -f "$daemon_source" ]; then
  command -v cargo >/dev/null 2>&1 || { echo 'Service binaries are missing and Cargo is not installed.' >&2; exit 1; }
  cargo build --release --no-default-features --features admin,daemon --bin db-operator-admin --bin db-operator-daemon --manifest-path "$skill_dir/native/Cargo.toml"
  admin_source="$skill_dir/native/target/release/db-operator-admin"
  daemon_source="$skill_dir/native/target/release/db-operator-daemon"
fi

if [ "$platform" = linux ]; then
  getent group "$agent_group" >/dev/null 2>&1 || groupadd --system "$agent_group"
  id "$service_user" >/dev/null 2>&1 || useradd --system --home-dir "$data_dir" --shell /usr/sbin/nologin "$service_user"
  usermod -a -G "$agent_group" "$agent_user"
else
  dscl . -read "/Groups/$agent_group" >/dev/null 2>&1 || dscl . -create "/Groups/$agent_group"
  dscl . -read "/Users/$service_user" >/dev/null 2>&1 || {
    dscl . -create "/Users/$service_user"
    dscl . -create "/Users/$service_user" UserShell /usr/bin/false
    dscl . -create "/Users/$service_user" NFSHomeDirectory "$data_dir"
  }
  dseditgroup -o edit -a "$agent_user" -t user "$agent_group"
fi

mkdir -p "$install_dir" "$data_dir" "$runtime_dir" "$(dirname "$agent_config")"
install -m 0755 "$admin_source" "$install_dir/db-operator-admin"
install -m 0755 "$daemon_source" "$install_dir/db-operator-daemon"
chown -R "$service_user:$agent_group" "$data_dir" "$runtime_dir"
chmod 700 "$data_dir"
chmod 2770 "$runtime_dir"

run_admin() {
  if [ "$platform" = linux ]; then
    runuser -u "$service_user" -- env DB_OPERATOR_HOME="$data_dir" "$install_dir/db-operator-admin" "$@"
  else
    sudo -u "$service_user" env DB_OPERATOR_HOME="$data_dir" "$install_dir/db-operator-admin" "$@"
  fi
}

if [ "$register" -eq 1 ]; then
  run_admin register
fi
temporary_client="$data_dir/client.json"
run_admin client issue --connection "$connection" --endpoint "$endpoint" --output "$temporary_client"
agent_primary_group=$(id -gn "$agent_user")
install -d -o "$agent_user" -g "$agent_primary_group" -m 0700 "$(dirname "$agent_config")"
install -o "$agent_user" -g "$agent_primary_group" -m 0600 "$temporary_client" "$agent_config"
rm -f "$temporary_client"

if [ "$platform" = linux ]; then
  cat > /etc/systemd/system/db-operator.service <<EOF
[Unit]
Description=DB Operator Security Daemon
After=network-online.target

[Service]
Type=simple
User=$service_user
Group=$agent_group
RuntimeDirectory=db-operator
RuntimeDirectoryMode=0770
ExecStart=$install_dir/db-operator-daemon --home $data_dir --endpoint $endpoint
Restart=on-failure
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ReadWritePaths=$data_dir $runtime_dir

[Install]
WantedBy=multi-user.target
EOF
  systemctl daemon-reload
  systemctl enable --now db-operator.service
else
  launchd_plist=/Library/LaunchDaemons/com.al.db-operator.plist
  cat > "$launchd_plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>com.al.db-operator</string>
<key>UserName</key><string>$service_user</string>
<key>GroupName</key><string>$agent_group</string>
<key>ProgramArguments</key><array>
<string>$install_dir/db-operator-daemon</string><string>--home</string><string>$data_dir</string>
<string>--endpoint</string><string>$endpoint</string>
</array>
<key>KeepAlive</key><true/><key>RunAtLoad</key><true/>
</dict></plist>
EOF
  chmod 0644 "$launchd_plist"
  launchctl bootout system/com.al.db-operator >/dev/null 2>&1 || true
  launchctl bootstrap system "$launchd_plist"
fi

printf 'Installed DB Operator service and Agent client config: %s\n' "$agent_config"
