#!/bin/bash
# ╔══════════════════════════════════════════════╗
# ║        bolly — AI companion uninstaller       ║
# ╚══════════════════════════════════════════════╝
#
# Usage:
#   curl -fsSL https://bollyai.dev/uninstall.sh | bash
#
# Options (env vars):
#   BOLLY_DIR=/custom/path   Uninstall from custom directory (default: ~/.bolly)
#   KEEP_DATA=1              Keep user data (instances, config) — only remove binary + service
#
set -e

# ─── Colors ───────────────────────────────────────────────────────────────────
BOLD='\033[1m'
DIM='\033[2m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
CYAN='\033[0;36m'
RED='\033[0;31m'
NC='\033[0m'

log()  { echo -e "  ${GREEN}✓${NC} $1"; }
info() { echo -e "  ${DIM}$1${NC}"; }
warn() { echo -e "  ${YELLOW}!${NC} $1"; }
fail() { echo -e "  ${RED}✗${NC} $1"; exit 1; }
step() { echo -e "\n${CYAN}${BOLD}$1${NC}"; }

# ─── Banner ───────────────────────────────────────────────────────────────────
echo ""
echo -e "${BOLD}  ┌─────────────────────────────┐${NC}"
echo -e "${BOLD}  │${NC}     ${CYAN}bolly${NC} uninstaller       ${BOLD}│${NC}"
echo -e "${BOLD}  └─────────────────────────────┘${NC}"

# ─── Detect platform ─────────────────────────────────────────────────────────
OS="$(uname -s)"
case "$OS" in
    Linux)   PLATFORM="linux" ;;
    Darwin)  PLATFORM="macos" ;;
    *)       fail "unsupported OS: $OS" ;;
esac

# ─── Config ───────────────────────────────────────────────────────────────────
BOLLY_DIR="${BOLLY_DIR:-$HOME/.bolly}"
BIN_DIR="$BOLLY_DIR/bin"
BIN="$BIN_DIR/bolly"
KEEP_DATA="${KEEP_DATA:-}"

if [ ! -d "$BOLLY_DIR" ]; then
    fail "bolly directory not found at $BOLLY_DIR — nothing to uninstall"
fi

# ─── Interactive prompt ──────────────────────────────────────────────────────
if [ -z "$KEEP_DATA" ] && [ -t 1 ] && [ -e /dev/tty ]; then
    # Terminal available — ask user (read from /dev/tty so it works with curl | bash)
    echo ""
    echo -e "  Your data is at ${BOLD}$BOLLY_DIR/data/${NC}"
    echo -e "  This includes config, memories, chats, and uploads."
    echo ""
    echo -e "  ${BOLD}1)${NC} Remove everything (binary + data)"
    echo -e "  ${BOLD}2)${NC} Keep my data (only remove binary + service)"
    echo ""
    printf "  Choose [1/2]: "
    read -r choice < /dev/tty
    case "$choice" in
        2) KEEP_DATA=1 ;;
        *) KEEP_DATA=0 ;;
    esac
elif [ -z "$KEEP_DATA" ]; then
    # No terminal at all — default to keep data (safe default)
    KEEP_DATA=1
fi

# ─── Stop running service ────────────────────────────────────────────────────
step "stopping bolly"

if [ "$PLATFORM" = "macos" ]; then
    PLIST="$HOME/Library/LaunchAgents/dev.bollyai.bolly.plist"
    if [ -f "$PLIST" ]; then
        launchctl unload "$PLIST" 2>/dev/null || true
        log "launchd service unloaded"
    fi
elif [ "$PLATFORM" = "linux" ]; then
    if command -v systemctl &>/dev/null; then
        if [ "$(id -u)" -eq 0 ] && [ -f "/etc/systemd/system/bolly.service" ]; then
            systemctl stop bolly 2>/dev/null || true
            systemctl disable bolly 2>/dev/null || true
            log "systemd service stopped"
        elif [ -f "$HOME/.config/systemd/user/bolly.service" ]; then
            systemctl --user stop bolly 2>/dev/null || true
            systemctl --user disable bolly 2>/dev/null || true
            log "user systemd service stopped"
        fi
    fi
fi

# Kill any remaining bolly process
OLD_PID=$(pgrep -f "$BIN_DIR/bolly" 2>/dev/null | head -1)
if [ -n "$OLD_PID" ]; then
    kill "$OLD_PID" 2>/dev/null || true
    sleep 1
    kill -9 "$OLD_PID" 2>/dev/null || true
    log "stopped running process (PID: $OLD_PID)"
fi

# ─── Remove service files ────────────────────────────────────────────────────
step "removing service files"

if [ "$PLATFORM" = "macos" ]; then
    PLIST="$HOME/Library/LaunchAgents/dev.bollyai.bolly.plist"
    if [ -f "$PLIST" ]; then
        rm -f "$PLIST"
        log "removed $PLIST"
    else
        info "no launchd plist found"
    fi
elif [ "$PLATFORM" = "linux" ]; then
    if [ "$(id -u)" -eq 0 ] && [ -f "/etc/systemd/system/bolly.service" ]; then
        rm -f "/etc/systemd/system/bolly.service"
        systemctl daemon-reload 2>/dev/null || true
        log "removed /etc/systemd/system/bolly.service"
    elif [ -f "$HOME/.config/systemd/user/bolly.service" ]; then
        rm -f "$HOME/.config/systemd/user/bolly.service"
        systemctl --user daemon-reload 2>/dev/null || true
        log "removed user systemd service"
    else
        info "no systemd service found"
    fi
fi

# ─── Remove files ────────────────────────────────────────────────────────────
step "removing files"

if [ "$KEEP_DATA" = "1" ]; then
    # Only remove binary and bin directory
    rm -rf "$BIN_DIR"
    rm -f "$BOLLY_DIR/bolly.log"
    log "removed binary and logs"
    info "kept user data at $BOLLY_DIR/data/"
else
    rm -rf "$BOLLY_DIR"
    log "removed $BOLLY_DIR"
fi

# ─── Clean PATH from shell rc ────────────────────────────────────────────────
step "cleaning shell config"

SHELL_NAME=$(basename "$SHELL" 2>/dev/null || echo "bash")
RC_FILE="$HOME/.${SHELL_NAME}rc"

if [ -f "$RC_FILE" ] && grep -q "$BIN_DIR" "$RC_FILE"; then
    # Remove the bolly PATH lines (comment + export)
    sed -i.bak '/# bolly/d' "$RC_FILE"
    sed -i.bak "\|$BIN_DIR|d" "$RC_FILE"
    rm -f "${RC_FILE}.bak"
    log "removed PATH entry from $RC_FILE"
else
    info "no PATH entry found in $RC_FILE"
fi

# ─── Done ─────────────────────────────────────────────────────────────────────
echo ""
echo -e "${BOLD}  ┌─────────────────────────────┐${NC}"
echo -e "${BOLD}  │${NC}  ${GREEN}bolly uninstalled${NC}          ${BOLD}│${NC}"
echo -e "${BOLD}  └─────────────────────────────┘${NC}"
echo ""
if [ "$KEEP_DATA" = "1" ]; then
    echo -e "  ${DIM}your data is still at ${BOLD}$BOLLY_DIR/data/${NC}"
    echo -e "  ${DIM}to remove it: ${BOLD}rm -rf $BOLLY_DIR${NC}"
fi
echo ""
