#!/bin/sh
set -e

PERSIST_DIR="${BOLLY_HOME:-/data}"
BIN_DIR="$PERSIST_DIR/bin"
BINARY="$BIN_DIR/bolly"

mkdir -p "$PERSIST_DIR" "$BIN_DIR"

# --- Download binary if not present ---
if [ ! -x "$BINARY" ]; then
    echo "[entrypoint] downloading bolly binary..."
    /opt/bolly/scripts/update-bolly.sh
fi

# --- Background update check (non-blocking) ---
/opt/bolly/scripts/update-bolly.sh &

# --- Ensure Playwright on persistent volume ---
if [ ! -d "$PERSIST_DIR/.playwright" ]; then
    echo "[entrypoint] installing Playwright Chromium..."
    PLAYWRIGHT_BROWSERS_PATH="$PERSIST_DIR/.playwright" npx playwright@1.52.0 install --with-deps chromium 2>/dev/null || true
fi
export PLAYWRIGHT_BROWSERS_PATH="$PERSIST_DIR/.playwright"

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
