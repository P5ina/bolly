#!/bin/sh
set -e

PERSIST_DIR="${BOLLY_HOME:-/data}"
BIN_DIR="$PERSIST_DIR/bin"
PERSISTENT_BINARY="$BIN_DIR/bolly"
PKG_LIST="$PERSIST_DIR/.apt-packages"
PIP_LIST="$PERSIST_DIR/.pip-packages"
NPM_LIST="$PERSIST_DIR/.npm-packages"

# --- Try to update binary from GitHub releases ---
# Run in background so startup isn't blocked if network is slow.
# If update succeeds, next restart will use it.
if [ -f /opt/bolly/scripts/update-bolly.sh ]; then
    /opt/bolly/scripts/update-bolly.sh &
fi


# --- Restore apt packages ---
if [ -f "$PKG_LIST" ] && [ -s "$PKG_LIST" ]; then
    echo "[entrypoint] restoring apt packages..."
    apt-get update -qq 2>/dev/null
    xargs -a "$PKG_LIST" apt-get install -y --no-install-recommends -qq 2>/dev/null || true
    rm -rf /var/lib/apt/lists/*
fi

# --- Restore pip packages ---
if [ -f "$PIP_LIST" ] && [ -s "$PIP_LIST" ]; then
    echo "[entrypoint] restoring pip packages..."
    pip install --break-system-packages -q -r "$PIP_LIST" 2>/dev/null || true
fi

# --- Restore npm global packages ---
if [ -f "$NPM_LIST" ] && [ -s "$NPM_LIST" ]; then
    echo "[entrypoint] restoring npm packages..."
    xargs -a "$NPM_LIST" npm install -g --silent 2>/dev/null || true
fi

# --- Ensure Playwright Chromium is installed ---
if ! command -v chromium >/dev/null 2>&1 && [ ! -d "$PERSIST_DIR/.playwright" ]; then
    echo "[entrypoint] installing Playwright Chromium..."
    PLAYWRIGHT_BROWSERS_PATH="$PERSIST_DIR/.playwright" npx playwright@1.52.0 install --with-deps chromium 2>/dev/null || true
fi
export PLAYWRIGHT_BROWSERS_PATH="$PERSIST_DIR/.playwright"

# --- Setup config ---
mkdir -p "$PERSIST_DIR"

# --- Install wrapper scripts ---
cat > /usr/local/bin/persist-apt <<'WRAPPER'
#!/bin/sh
/usr/bin/apt-get "$@"
STATUS=$?
if [ $STATUS -eq 0 ] && [ "$1" = "install" ]; then
    PERSIST_DIR="${BOLLY_HOME:-/data}"
    PKG_LIST="$PERSIST_DIR/.apt-packages"
    shift
    for arg in "$@"; do
        case "$arg" in
            -*) continue ;;
            *) echo "$arg" >> "$PKG_LIST" ;;
        esac
    done
    if [ -f "$PKG_LIST" ]; then
        sort -u "$PKG_LIST" -o "$PKG_LIST"
    fi
fi
exit $STATUS
WRAPPER
chmod +x /usr/local/bin/persist-apt

cat > /usr/local/bin/persist-pip <<'WRAPPER'
#!/bin/sh
/usr/bin/pip3 "$@" || /usr/bin/pip "$@"
STATUS=$?
if [ $STATUS -eq 0 ] && [ "$1" = "install" ]; then
    PERSIST_DIR="${BOLLY_HOME:-/data}"
    PIP_LIST="$PERSIST_DIR/.pip-packages"
    shift
    for arg in "$@"; do
        case "$arg" in
            -*) continue ;;
            *) echo "$arg" >> "$PIP_LIST" ;;
        esac
    done
    if [ -f "$PIP_LIST" ]; then
        sort -u "$PIP_LIST" -o "$PIP_LIST"
    fi
fi
exit $STATUS
WRAPPER
chmod +x /usr/local/bin/persist-pip

# --- Choose binary: persistent (updated) or bundled (fallback) ---
if [ -x "$PERSISTENT_BINARY" ]; then
    echo "[entrypoint] using persistent binary: $(cat "$BIN_DIR/.version" 2>/dev/null || echo 'unknown')"
    exec "$PERSISTENT_BINARY"
else
    echo "[entrypoint] using bundled binary"
    exec bolly
fi
