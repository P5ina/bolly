#!/bin/sh
# Download latest bolly binary from GitHub releases to persistent storage.
# Called by entrypoint on startup, or manually to update.
set -e

PERSIST_DIR="${BOLLY_HOME:-/data}"
BIN_DIR="$PERSIST_DIR/bin"
BINARY="$BIN_DIR/bolly"
VERSION_FILE="$BIN_DIR/.version"
REPO="triangle-int/bolly"

# Determine target architecture
ARCH=$(uname -m)
case "$ARCH" in
    x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
    aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
    *) echo "[update] unsupported arch: $ARCH"; exit 1 ;;
esac

mkdir -p "$BIN_DIR"

# Get latest release tag
LATEST=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"//;s/".*//')
if [ -z "$LATEST" ]; then
    echo "[update] could not fetch latest version"
    exit 1
fi

# Check if already up to date
if [ -f "$VERSION_FILE" ] && [ "$(cat "$VERSION_FILE")" = "$LATEST" ] && [ -f "$BINARY" ]; then
    echo "[update] already at $LATEST"
    exit 0
fi

echo "[update] downloading bolly $LATEST for $TARGET..."
ASSET_URL="https://github.com/$REPO/releases/download/$LATEST/bolly-$TARGET"

if curl -fsSL "$ASSET_URL" -o "$BINARY.tmp"; then
    chmod +x "$BINARY.tmp"
    mv "$BINARY.tmp" "$BINARY"
    echo "$LATEST" > "$VERSION_FILE"
    echo "[update] updated to $LATEST"
else
    echo "[update] download failed, keeping current binary"
    rm -f "$BINARY.tmp"
    exit 1
fi
