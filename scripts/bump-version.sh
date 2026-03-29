#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION_FILE="$ROOT/VERSION"

if [ $# -eq 1 ]; then
  echo "$1" > "$VERSION_FILE"
fi

VERSION="$(cat "$VERSION_FILE" | tr -d '[:space:]')"

if [ -z "$VERSION" ]; then
  echo "error: VERSION file is empty" >&2
  exit 1
fi

# server/Cargo.toml
sed -i '' -E 's/^version = ".*"/version = "'"$VERSION"'"/' "$ROOT/server/Cargo.toml"

# client/package.json
sed -i '' -E 's/"version": ".*"/"version": "'"$VERSION"'"/' "$ROOT/client/package.json"

# landing/package.json
sed -i '' -E 's/"version": ".*"/"version": "'"$VERSION"'"/' "$ROOT/landing/package.json"

# desktop/package.json
sed -i '' -E 's/"version": ".*"/"version": "'"$VERSION"'"/' "$ROOT/desktop/package.json"

# desktop/src-tauri/Cargo.toml
sed -i '' -E 's/^version = ".*"/version = "'"$VERSION"'"/' "$ROOT/desktop/src-tauri/Cargo.toml"

# desktop/src-tauri/tauri.conf.json
sed -i '' -E 's/"version": ".*"/"version": "'"$VERSION"'"/' "$ROOT/desktop/src-tauri/tauri.conf.json"

# Update Cargo.lock
(cd "$ROOT/server" && cargo check --quiet 2>/dev/null || true)

echo "v$VERSION"
