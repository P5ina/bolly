#!/bin/sh
# Download bolly binary from GitHub releases to persistent storage.
# Supports two channels:
#   BOLLY_CHANNEL=stable  → downloads from releases/latest (default)
#   BOLLY_CHANNEL=nightly → downloads from releases/tag/nightly
set -e

PERSIST_DIR="${BOLLY_HOME:-/data}"
BIN_DIR="$PERSIST_DIR/bin"
BINARY="$BIN_DIR/bolly"
VERSION_FILE="$BIN_DIR/.version"
REPO="triangle-int/bolly"
RELEASE_TOKEN="${BOLLY_RELEASE_TOKEN:-}"
AUTH_HEADER=""
if [ -n "$RELEASE_TOKEN" ]; then
    AUTH_HEADER="Authorization: token $RELEASE_TOKEN"
fi
# Read channel from persistent file, default to stable
if [ -f "$PERSIST_DIR/.update-channel" ]; then
    CHANNEL=$(cat "$PERSIST_DIR/.update-channel" | tr -d '[:space:]')
fi
CHANNEL="${CHANNEL:-stable}"

# Determine target architecture
ARCH=$(uname -m)
case "$ARCH" in
    x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
    aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
    *) echo "[update] unsupported arch: $ARCH"; exit 1 ;;
esac

mkdir -p "$BIN_DIR"

# Get release info based on channel
if [ "$CHANNEL" = "nightly" ]; then
    API_URL="https://api.github.com/repos/$REPO/releases/tags/nightly"
else
    API_URL="https://api.github.com/repos/$REPO/releases/latest"
fi

RELEASE_JSON=$(curl -fsSL ${AUTH_HEADER:+-H "$AUTH_HEADER"} "$API_URL" 2>/dev/null) || { echo "[update] could not fetch release info"; exit 1; }
TAG=$(echo "$RELEASE_JSON" | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"//;s/".*//')

if [ -z "$TAG" ]; then
    echo "[update] no tag found in release"
    exit 1
fi

# For nightly, use the published_at timestamp as version (tag is always "nightly")
if [ "$CHANNEL" = "nightly" ]; then
    PUBLISHED=$(echo "$RELEASE_JSON" | grep '"published_at"' | head -1 | sed 's/.*"published_at": *"//;s/".*//')
    VERSION="nightly-$PUBLISHED"
else
    VERSION="$TAG"
fi

# Check if already up to date
if [ -f "$VERSION_FILE" ] && [ "$(cat "$VERSION_FILE")" = "$VERSION" ] && [ -f "$BINARY" ]; then
    echo "[update] already at $VERSION"
    exit 2  # exit 2 = no update needed (distinct from 0 = updated, 1 = error)
fi

echo "[update] downloading bolly $VERSION ($CHANNEL) for $TARGET..."

# For private repos, browser_download_url returns 404.
# Use GitHub API asset endpoint with Accept: application/octet-stream instead.
# See: https://docs.github.com/en/rest/releases/assets#get-a-release-asset
ASSET_NAME="bolly-$TARGET"
ASSET_API_URL=""
if [ -n "$RELEASE_TOKEN" ] && command -v jq >/dev/null 2>&1; then
    ASSET_API_URL=$(echo "$RELEASE_JSON" | jq -r ".assets[] | select(.name == \"$ASSET_NAME\") | .url" 2>/dev/null) || true
fi

DOWNLOAD_OK=false
if [ -n "$ASSET_API_URL" ] && [ "$ASSET_API_URL" != "null" ] && [ -n "$RELEASE_TOKEN" ]; then
    # Private repo: download via API with octet-stream accept header
    echo "[update] downloading via GitHub API (private repo)"
    if curl -fSL --max-time 120 \
        -H "$AUTH_HEADER" \
        -H "Accept: application/octet-stream" \
        "$ASSET_API_URL" -o "$BINARY.tmp" 2>&1; then
        DOWNLOAD_OK=true
    else
        echo "[update] curl failed (exit $?), URL: $ASSET_API_URL"
    fi
else
    # Public repo fallback: direct download URL
    ASSET_URL="https://github.com/$REPO/releases/download/$TAG/$ASSET_NAME"
    echo "[update] downloading via direct URL"
    if curl -fSL --max-time 120 ${AUTH_HEADER:+-H "$AUTH_HEADER"} "$ASSET_URL" -o "$BINARY.tmp" 2>&1; then
        DOWNLOAD_OK=true
    else
        echo "[update] curl failed (exit $?), URL: $ASSET_URL"
    fi
fi

# Verify downloaded file is a real binary (not an HTML error page)
if [ "$DOWNLOAD_OK" = "true" ]; then
    FILE_SIZE=$(wc -c < "$BINARY.tmp" 2>/dev/null || echo 0)
    if [ "$FILE_SIZE" -lt 1000000 ]; then
        echo "[update] downloaded file too small (${FILE_SIZE} bytes), likely an error page"
        head -c 200 "$BINARY.tmp" 2>/dev/null || true
        echo ""
        DOWNLOAD_OK=false
    fi
fi

if [ "$DOWNLOAD_OK" = "true" ]; then
    chmod +x "$BINARY.tmp"
    mv "$BINARY.tmp" "$BINARY"
    echo "$VERSION" > "$VERSION_FILE"
    echo "[update] updated to $VERSION"
else
    echo "[update] download failed, keeping current binary"
    rm -f "$BINARY.tmp"
    exit 1
fi
