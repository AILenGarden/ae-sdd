#!/usr/bin/env sh
set -eu

skill_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
bin_dir="$skill_dir/bin"
destination="$bin_dir/db-operator"

case "$(uname -s)" in
  Darwin) platform=macos ;;
  Linux) platform=linux ;;
  *) echo "Unsupported operating system: $(uname -s)" >&2; exit 1 ;;
esac
case "$(uname -m)" in
  x86_64|amd64) architecture=x86_64 ;;
  arm64|aarch64) architecture=aarch64 ;;
  *) echo "Unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

prebuilt="$bin_dir/$platform-$architecture/db-operator"
mkdir -p "$bin_dir"
if [ "${1:-}" != "--build-from-source" ] && [ -f "$prebuilt" ]; then
  cp "$prebuilt" "$destination"
elif [ ! -f "$destination" ] || [ "${1:-}" = "--build-from-source" ]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "No prebuilt Agent client exists for $platform-$architecture and Cargo is not installed." >&2
    exit 1
  fi
  cargo build --release --no-default-features --features client --bin db-operator --manifest-path "$skill_dir/native/Cargo.toml"
  cp "$skill_dir/native/target/release/db-operator" "$destination"
fi
chmod 700 "$destination"
"$destination" --version
printf 'Installed query-only db-operator client: %s\n' "$destination"
printf '%s\n' 'Client installation only: the daemon service and client.json are not configured by this script.'
printf '%s\n' 'An administrator should follow references/connection-schema.md and use service components matching this operating system and architecture.'
