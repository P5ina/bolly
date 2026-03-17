#!/bin/sh
set -e

PERSIST_DIR="${BOLLY_HOME:-/data}"
BIN_DIR="$PERSIST_DIR/bin"
BINARY="$BIN_DIR/bolly"
SCRIPTS_DIR="$PERSIST_DIR/scripts"
REPO="triangle-int/bolly"

mkdir -p "$PERSIST_DIR" "$BIN_DIR" "$SCRIPTS_DIR"

# --- Self-update entrypoint and scripts from GitHub ---
# This ensures new entrypoints take effect without image rebuild
BRANCH="${BOLLY_CHANNEL:-stable}"
if [ "$BRANCH" = "stable" ]; then BRANCH="main"; fi
for SCRIPT in entrypoint.sh update-bolly.sh; do
    curl -fsSL "https://raw.githubusercontent.com/$REPO/$BRANCH/server/scripts/$SCRIPT" \
        -o "$SCRIPTS_DIR/$SCRIPT.tmp" 2>/dev/null && \
        chmod +x "$SCRIPTS_DIR/$SCRIPT.tmp" && \
        mv "$SCRIPTS_DIR/$SCRIPT.tmp" "$SCRIPTS_DIR/$SCRIPT" || true
done

# If we downloaded a newer entrypoint, re-exec into it (once)
if [ -f "$SCRIPTS_DIR/entrypoint.sh" ] && [ "$BOLLY_ENTRYPOINT_UPDATED" != "1" ]; then
    export BOLLY_ENTRYPOINT_UPDATED=1
    exec "$SCRIPTS_DIR/entrypoint.sh"
fi

# --- Download binary if not present ---
UPDATE_SCRIPT="$SCRIPTS_DIR/update-bolly.sh"
[ -x "$UPDATE_SCRIPT" ] || UPDATE_SCRIPT="/opt/bolly/scripts/update-bolly.sh"

if [ ! -x "$BINARY" ]; then
    echo "[entrypoint] downloading bolly binary..."
    "$UPDATE_SCRIPT"
fi

# --- Background update check (non-blocking) ---
"$UPDATE_SCRIPT" &

# --- Ensure Chromium is available ---
CHROMIUM_BIN=$(command -v chromium-browser 2>/dev/null || command -v chromium 2>/dev/null || echo "")
if [ -z "$CHROMIUM_BIN" ]; then
    echo "[entrypoint] installing Chromium..."
    apt-get update -qq 2>/dev/null
    apt-get install -y --no-install-recommends chromium-browser 2>/dev/null || \
    apt-get install -y --no-install-recommends chromium 2>/dev/null || true
    rm -rf /var/lib/apt/lists/*
    CHROMIUM_BIN=$(command -v chromium-browser 2>/dev/null || command -v chromium 2>/dev/null || echo "")
fi
export CHROMIUM_PATH="$CHROMIUM_BIN"

# --- Restore user-installed packages ---
if [ -f "$PERSIST_DIR/.apt-packages" ] && [ -s "$PERSIST_DIR/.apt-packages" ]; then
    echo "[entrypoint] restoring apt packages..."
    apt-get update -qq 2>/dev/null
    xargs -a "$PERSIST_DIR/.apt-packages" apt-get install -y --no-install-recommends -qq 2>/dev/null || true
    rm -rf /var/lib/apt/lists/*
fi

if [ -f "$PERSIST_DIR/.pip-packages" ] && [ -s "$PERSIST_DIR/.pip-packages" ]; then
    echo "[entrypoint] restoring pip packages..."
    pip install --break-system-packages -q -r "$PERSIST_DIR/.pip-packages" 2>/dev/null || true
fi

if [ -f "$PERSIST_DIR/.npm-packages" ] && [ -s "$PERSIST_DIR/.npm-packages" ]; then
    echo "[entrypoint] restoring npm packages..."
    xargs -a "$PERSIST_DIR/.npm-packages" npm install -g --silent 2>/dev/null || true
fi

# --- persist-apt / persist-pip wrappers ---
cat > /usr/local/bin/persist-apt <<'WRAPPER'
#!/bin/sh
/usr/bin/apt-get "$@"
STATUS=$?
if [ $STATUS -eq 0 ] && [ "$1" = "install" ]; then
    PKG_LIST="${BOLLY_HOME:-/data}/.apt-packages"
    shift
    for arg in "$@"; do case "$arg" in -*) continue ;; *) echo "$arg" >> "$PKG_LIST" ;; esac; done
    [ -f "$PKG_LIST" ] && sort -u "$PKG_LIST" -o "$PKG_LIST"
fi
exit $STATUS
WRAPPER
chmod +x /usr/local/bin/persist-apt

cat > /usr/local/bin/persist-pip <<'WRAPPER'
#!/bin/sh
/usr/bin/pip3 "$@" || /usr/bin/pip "$@"
STATUS=$?
if [ $STATUS -eq 0 ] && [ "$1" = "install" ]; then
    PIP_LIST="${BOLLY_HOME:-/data}/.pip-packages"
    shift
    for arg in "$@"; do case "$arg" in -*) continue ;; *) echo "$arg" >> "$PIP_LIST" ;; esac; done
    [ -f "$PIP_LIST" ] && sort -u "$PIP_LIST" -o "$PIP_LIST"
fi
exit $STATUS
WRAPPER
chmod +x /usr/local/bin/persist-pip

# --- Start ---
echo "[entrypoint] starting bolly $(cat "$BIN_DIR/.version" 2>/dev/null || echo '')"
exec "$BINARY"
