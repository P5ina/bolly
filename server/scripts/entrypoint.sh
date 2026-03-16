#!/bin/sh
set -e

PERSIST_DIR="${BOLLY_HOME:-/data}"
PKG_LIST="$PERSIST_DIR/.apt-packages"
PIP_LIST="$PERSIST_DIR/.pip-packages"
NPM_LIST="$PERSIST_DIR/.npm-packages"

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

# --- Setup config ---
mkdir -p "$PERSIST_DIR"
grep -q static_dir "$PERSIST_DIR/config.toml" 2>/dev/null || printf 'static_dir = "/opt/bolly/static"\n' >> "$PERSIST_DIR/config.toml"

# --- Install wrapper scripts ---
# These wrappers call the real command AND persist the package list

cat > /usr/local/bin/persist-apt <<'WRAPPER'
#!/bin/sh
# Run real apt-get with all arguments
/usr/bin/apt-get "$@"
STATUS=$?
# If install succeeded, save package names
if [ $STATUS -eq 0 ] && [ "$1" = "install" ]; then
    PERSIST_DIR="${BOLLY_HOME:-/data}"
    PKG_LIST="$PERSIST_DIR/.apt-packages"
    shift  # remove "install"
    for arg in "$@"; do
        case "$arg" in
            -*) continue ;;  # skip flags
            *) echo "$arg" >> "$PKG_LIST" ;;
        esac
    done
    # Deduplicate
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

echo "[entrypoint] starting bolly..."
exec bolly
