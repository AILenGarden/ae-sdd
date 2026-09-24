#!/usr/bin/env sh
# Build a macOS or Linux release package from a source tree or a checkout. Cargo is
# required here, on the build host only: the package ships prebuilt binaries and
# installing it on a target machine needs no compiler.
set -eu

output_root=${1:-}
if [ -z "$output_root" ]; then
  echo 'Usage: package-unix.sh OUTPUT_DIR [--skip-build]' >&2
  exit 2
fi
skip_build=0
if [ "${2:-}" = "--skip-build" ]; then
  skip_build=1
fi

case "$(uname -s)" in
  Darwin) platform=macos ;;
  Linux) platform=linux ;;
  *) echo "Only macOS and Linux build hosts are supported: $(uname -s)" >&2; exit 1 ;;
esac
case "$(uname -m)" in
  x86_64|amd64) architecture=x86_64 ;;
  arm64|aarch64) architecture=aarch64 ;;
  *) echo "Unsupported architecture: $(uname -m)" >&2; exit 1 ;;
esac

source_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
manifest="$source_root/native/Cargo.toml"
version=$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$manifest" | head -n 1)
if [ -z "$version" ]; then
  echo 'Package version is missing.' >&2
  exit 1
fi
stamp=$(date -u +%Y%m%dT%H%M%SZ)
package_name="db-operator-$version-$platform-$architecture-$stamp"
package_root="$output_root/$package_name"

if [ "$skip_build" -eq 0 ]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo 'Cargo is required to build the release binaries.' >&2
    exit 1
  fi
  cargo build --locked --release --manifest-path "$manifest" --no-default-features --features client --bin db-operator
  cargo build --locked --release --manifest-path "$manifest" --no-default-features --features admin,daemon --bin db-operator-admin --bin db-operator-daemon
fi
release_dir="$source_root/native/target/release"

# Explicit payload; never copy a workspace, target directory, or user's state.
client_bin="$release_dir/db-operator"
admin_bin="$release_dir/db-operator-admin"
daemon_bin="$release_dir/db-operator-daemon"
for binary in "$client_bin" "$admin_bin" "$daemon_bin"; do
  if [ ! -f "$binary" ]; then
    echo "Missing release binary: $binary" >&2
    exit 1
  fi
done
if [ -e "$package_root" ]; then
  echo 'Output already exists; refusing to replace it.' >&2
  exit 1
fi
mkdir -p "$package_root/bin/$platform-$architecture" "$package_root/bin/service/$platform-$architecture" \
  "$package_root/scripts" "$package_root/references" "$package_root/ui"

cp "$client_bin" "$package_root/bin/db-operator"
cp "$client_bin" "$package_root/bin/$platform-$architecture/db-operator"
cp "$admin_bin" "$package_root/bin/service/$platform-$architecture/db-operator-admin"
cp "$daemon_bin" "$package_root/bin/service/$platform-$architecture/db-operator-daemon"
# Shell scripts are normalized to LF: a source tree exported from Windows may
# carry CRLF, and a CRLF shebang line fails on the target machine.
for name in install.sh install-service.sh open-registry.sh; do
  tr -d '\r' <"$source_root/scripts/$name" >"$package_root/scripts/$name"
done
cp "$source_root/scripts/registry-server.mjs" "$package_root/scripts/registry-server.mjs"
cp "$source_root/ui/index.html" "$package_root/ui/index.html"
cp "$source_root/SKILL.md" "$package_root/SKILL.md"
cp "$source_root/references/connection-schema.md" "$package_root/references/connection-schema.md"
cp "$source_root/references/release-readme.md" "$package_root/README.md"
chmod 0755 "$package_root/bin/db-operator" "$package_root/bin/$platform-$architecture/db-operator" \
  "$package_root/bin/service/$platform-$architecture/db-operator-admin" \
  "$package_root/bin/service/$platform-$architecture/db-operator-daemon" \
  "$package_root/scripts/"*.sh

# Every shipped script must stay LF so it runs on the target machine.
for script in "$package_root/scripts/"*.sh; do
  if LC_ALL=C grep -q "$(printf '\r')" "$script"; then
    echo "CRLF line endings are not allowed in $script" >&2
    exit 1
  fi
done

sha256_of() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | cut -d' ' -f 1
  else
    sha256sum "$1" | cut -d' ' -f 1
  fi
}

manifest_file="$package_root/release-manifest.json"
file_list="$package_root/.release-files"
( cd "$package_root" && find . -type f ! -name release-manifest.json | sed 's|^\./||' | LC_ALL=C sort ) >"$file_list"
{
  printf '{\n'
  printf '  "package": "%s",\n' "$package_name"
  printf '  "version": "%s",\n' "$version"
  printf '  "target": "%s-%s",\n' "$platform" "$architecture"
  printf '  "builtAtUtc": "%s",\n' "$stamp"
  printf '  "registrationDataIncluded": false,\n'
  printf '  "credentialsIncluded": false,\n'
  printf '  "uiMode": "local-admin-http; requires Node.js and administrator privileges",\n'
  printf '  "files": [\n'
  first=1
  while IFS= read -r file; do
    if [ "$first" -eq 0 ]; then printf ',\n'; fi
    first=0
    checksum=$(sha256_of "$package_root/$file")
    bytes=$(wc -c <"$package_root/$file" | tr -d ' ')
    printf '    {"path": "%s", "sha256": "%s", "bytes": %s}' "$file" "$checksum" "$bytes"
  done <"$file_list"
  printf '\n  ]\n}\n'
} >"$manifest_file"
rm -f "$file_list"

archive="$package_root.tar.gz"
(cd "$output_root" && tar -czf "$package_name.tar.gz" "$package_name")
archive_hash=$(sha256_of "$archive")
printf '%s  %s\n' "$archive_hash" "$(basename "$archive")" >"$archive.sha256"
echo "PACKAGE=$package_root"
echo "ARCHIVE=$archive"
echo "SHA256=$archive_hash"
