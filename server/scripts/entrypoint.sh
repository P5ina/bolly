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
