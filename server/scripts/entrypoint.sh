#!/bin/sh
set -e

PERSIST_DIR="${BOLLY_HOME:-/data}"
BIN_DIR="$PERSIST_DIR/bin"
BINARY="$BIN_DIR/bolly"

mkdir -p "$PERSIST_DIR" "$BIN_DIR"

# --- Always check for updates before starting ---
echo "[entrypoint] checking for updates..."
/opt/bolly/scripts/update-bolly.sh || UPDATE_EXIT=$?
UPDATE_EXIT=${UPDATE_EXIT:-0}
if [ $UPDATE_EXIT -eq 0 ]; then
    echo "[entrypoint] update applied successfully"
elif [ $UPDATE_EXIT -eq 2 ]; then
    echo "[entrypoint] already up to date"
else
    echo "[entrypoint] WARNING: update failed (exit $UPDATE_EXIT), starting with existing binary"
fi

# --- Ensure Chromium is available ---
# Playwright installs chromium to ~/.cache/ms-playwright/
CHROMIUM_BIN=$(find /root/.cache/ms-playwright -name "chrome" -o -name "chromium" 2>/dev/null | head -1)
# Fallback: system chromium (if not a snap stub)
if [ -z "$CHROMIUM_BIN" ]; then
    CHROMIUM_BIN=$(command -v chromium-browser 2>/dev/null || command -v chromium 2>/dev/null || echo "")
    # Test if it's a snap stub
    if [ -n "$CHROMIUM_BIN" ] && "$CHROMIUM_BIN" --version 2>&1 | grep -qi "snap"; then
        CHROMIUM_BIN=""
    fi
fi
if [ -z "$CHROMIUM_BIN" ]; then
    echo "[entrypoint] installing Chromium via Playwright..."
    npx playwright install --with-deps chromium 2>/dev/null || true
    CHROMIUM_BIN=$(find /root/.cache/ms-playwright -name "chrome" -o -name "chromium" 2>/dev/null | head -1)
fi
export CHROMIUM_PATH="${CHROMIUM_BIN:-}"
echo "[entrypoint] chromium: ${CHROMIUM_PATH:-NOT FOUND}"

# --- Start Qdrant sidecar (vector search) ---
mkdir -p "$PERSIST_DIR/qdrant"
if command -v qdrant >/dev/null 2>&1; then
    qdrant --storage-path "$PERSIST_DIR/qdrant" &
    QDRANT_PID=$!
    echo "[entrypoint] qdrant started (PID: $QDRANT_PID), storage: $PERSIST_DIR/qdrant"
    # Wait for Qdrant to be ready (gRPC port 6334)
    for i in $(seq 1 30); do
        if curl -sf http://localhost:6333/healthz >/dev/null 2>&1; then
            echo "[entrypoint] qdrant ready"
            break
        fi
        sleep 1
    done
else
    echo "[entrypoint] qdrant binary not found, skipping vector search"
fi

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
