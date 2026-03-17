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

# --- Start (restart loop — keeps container alive on binary restart) ---
while true; do
    echo "[entrypoint] starting bolly $(cat "$BIN_DIR/.version" 2>/dev/null || echo '')"
    "$BINARY"
    EXIT_CODE=$?
    echo "[entrypoint] bolly exited with code $EXIT_CODE, restarting in 2s..."
    sleep 2
    # Re-run update check before restart
    /opt/bolly/scripts/update-bolly.sh 2>/dev/null || true
done
