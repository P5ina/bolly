#!/bin/bash
# ╔══════════════════════════════════════════════╗
# ║          bolly — AI companion installer       ║
# ║        https://github.com/triangle-int/bolly  ║
# ╚══════════════════════════════════════════════╝
#
# Usage:
#   curl -fsSL https://bollyai.dev/install.sh | bash
#
# Options (env vars):
#   BOLLY_CHANNEL=nightly    Install nightly instead of stable
#   BOLLY_DIR=/custom/path   Install to custom directory (default: ~/.bolly)
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

# ─── Orb animation (frames extracted from orb-onboarding.mp4) ─────────────────
animate_orb() {
    local H=16  # fixed frame height

    # Helper: pad frame to fixed height, centered vertically
    show_frame() {
        printf '\033[2J\033[H'
        local content="$1"
        local lines
        lines=$(echo "$content" | wc -l)
        local pad=$(( (H - lines) / 2 ))
        for _ in $(seq 1 $pad); do echo; done
        echo "$content"
        local bottom=$(( H - lines - pad ))
        for _ in $(seq 1 $bottom); do echo; done
    }

    show_frame '                   .......
                 ....··....
                 ....··....
                   ......'
    sleep 0.12

    show_frame '                   ......
                ............
               ......··......
             ......·:++:·......
              .....··::··.....
                .............
                 ...........
                  ........'
    sleep 0.12

    show_frame '                   ......
                  ..·**:...
                  ..·::·...
                    ....'
    sleep 0.1

    show_frame '                        ..
                     ..:+·.
                 ...··+%*:·.
                .·:+*%#%*+:.
                ·+%%%***++:.
                ..··.·::···..
                     .....'
    sleep 0.12

    show_frame '               ......··.......
             ··...··:+++::::::··..
            .:+:·:+*%##%%*******+:..
             .:+**%#%***%%*:··:++:·.
          .··..·+%%##%%%%*+:··...
          ·**·..:*++++*%*:·...
           .·····++:::+*:..
              ....·::::·...'
    sleep 0.15

    show_frame '                .·:+***++:·.
             .·:+**%%%%%***+:·.
            .:**+:++++++++++**:..
          ..:**+::::·::++***+**+:.
          .:+*++++:·····:+*%++**+·
          .:**++*+:·····:+*%*+*+:·
           ·+%%*%*+:···:+*%%***:·
           .·*%##%%*++**%%****:.
            .·+**%%%%%%%%%**+:.
              .·::+****++::·.
                 ........'
    sleep 0.15

    show_frame '                .·:++**++:·.
             .·:*%%#####%%*+::.
           .·+**********%%%*+*+·.
          .+**+**++*%******%%***:.
          :%****++*++:::+***%%%%%·
          +%+*%*++:·....·:+**%%%%+
          +%+*%**:·.....·:+*%%%%#+
          ·***%%%*+::··::++*%%%%%:
          .:****%##%%******%%%%%+.
           .·+***********%%%%%*:.
             .·+*%%%%%%%%%%*+:.
                .··:+++::··.'
    sleep 0.2

    show_frame '                    ....
                .·:+****++:.
             .·:*%########%*+:.
           .:+******+++++**+***:.
          .+%***+::::··::+++**%%:.
          :%%***+::······:++*%***·
          :%#%**+::·······:+*%%+*:          bolly
          :*#%%%*::::·····:+%%#*+:          your AI companion
          ·+*%%%**+:::::::+*%#%++·
           ·:+%%%**++::++*%%#%+:·.
            .·:+*%%*****%%%%*+:..
              ..:+**%%%%%*+:·.
                 ..·:::··..'
    sleep 1.0
    printf '\033[2J\033[H'
}

# Only animate if terminal is interactive
if [ -t 1 ]; then
    animate_orb
fi

# ─── Banner ───────────────────────────────────────────────────────────────────
echo ""
echo -e "${BOLD}  ┌─────────────────────────────┐${NC}"
echo -e "${BOLD}  │${NC}     ${CYAN}bolly${NC} installer         ${BOLD}│${NC}"
echo -e "${BOLD}  │${NC}     ${DIM}your AI companion${NC}       ${BOLD}│${NC}"
echo -e "${BOLD}  └─────────────────────────────┘${NC}"

# ─── Detect platform ─────────────────────────────────────────────────────────
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
    Linux)   PLATFORM="linux" ;;
    Darwin)  PLATFORM="macos" ;;
    *)       fail "unsupported OS: $OS (bolly supports Linux and macOS)" ;;
esac

case "$ARCH" in
    x86_64|amd64)   ARCH="x86_64" ;;
    aarch64|arm64)   ARCH="aarch64" ;;
    *)               fail "unsupported architecture: $ARCH" ;;
esac

case "$PLATFORM-$ARCH" in
    linux-x86_64)    TARGET="x86_64-unknown-linux-gnu" ;;
    linux-aarch64)   TARGET="aarch64-unknown-linux-gnu" ;;
    macos-aarch64)   TARGET="aarch64-apple-darwin" ;;
    macos-x86_64)    fail "Intel Macs are not supported — bolly requires Apple Silicon (M1+)" ;;
    *)               fail "unsupported platform: $PLATFORM-$ARCH" ;;
esac

# ─── Config ───────────────────────────────────────────────────────────────────
REPO="triangle-int/bolly"
CHANNEL="${BOLLY_CHANNEL:-stable}"
BOLLY_DIR="${BOLLY_DIR:-$HOME/.bolly}"
BIN_DIR="$BOLLY_DIR/bin"
BIN="$BIN_DIR/bolly"

step "detecting environment"
log "platform: ${BOLD}$PLATFORM $ARCH${NC}"
log "channel: ${BOLD}$CHANNEL${NC}"
log "install dir: ${BOLD}$BOLLY_DIR${NC}"

# ─── Download binary ─────────────────────────────────────────────────────────
step "downloading bolly"

if [ "$CHANNEL" = "nightly" ]; then
    API_URL="https://api.github.com/repos/$REPO/releases/tags/nightly"
else
    API_URL="https://api.github.com/repos/$REPO/releases/latest"
fi

RELEASE_JSON=$(curl -fsSL "$API_URL" 2>/dev/null) || fail "could not fetch release info"
TAG=$(echo "$RELEASE_JSON" | grep '"tag_name"' | head -1 | sed 's/.*: "//;s/".*//')

if [ -z "$TAG" ] || [ "$TAG" = "null" ]; then
    fail "could not find a $CHANNEL release"
fi

ASSET_NAME="bolly-server-$TARGET"
ASSET_NAME_LEGACY="bolly-$TARGET"
DOWNLOAD_URL="https://github.com/$REPO/releases/download/$TAG/$ASSET_NAME"
DOWNLOAD_URL_LEGACY="https://github.com/$REPO/releases/download/$TAG/$ASSET_NAME_LEGACY"

mkdir -p "$BIN_DIR" "$BOLLY_DIR"

info "downloading $TAG for $TARGET..."
# Try new asset name first, fall back to legacy for older releases
if ! curl -fsSL --head "$DOWNLOAD_URL" >/dev/null 2>&1; then
    DOWNLOAD_URL="$DOWNLOAD_URL_LEGACY"
fi
curl -fL --progress-bar "$DOWNLOAD_URL" -o "$BIN" || \
    fail "download failed — check https://github.com/$REPO/releases"
chmod +x "$BIN"
echo "$TAG" > "$BIN_DIR/.version"

log "downloaded ${BOLD}$TAG${NC}"

# ─── Config file ──────────────────────────────────────────────────────────────
if [ ! -f "$BOLLY_DIR/config.toml" ]; then
    step "creating config"
    cat > "$BOLLY_DIR/config.toml" <<'CONF'
host = "0.0.0.0"
port = 26559
auth_token = ""

[llm]
provider = "anthropic"
model = "claude-sonnet-4-6"

[llm.tokens]
ANTHROPIC = ""       # Required — get key at https://console.anthropic.com
GOOGLE_AI = ""       # Optional — embeddings + media analysis
ELEVENLABS = ""      # Optional — text-to-speech
CONF
    log "created $BOLLY_DIR/config.toml"
else
    log "config already exists, skipping"
fi

# ─── Update script ────────────────────────────────────────────────────────────
cat > "$BIN_DIR/update" <<UPDATESCRIPT
#!/bin/bash
set -e
REPO="$REPO"
BIN="$BIN"
CHANNEL="\${BOLLY_CHANNEL:-$CHANNEL}"
TARGET="$TARGET"
if [ "\$CHANNEL" = "nightly" ]; then
    API_URL="https://api.github.com/repos/\$REPO/releases/tags/nightly"
else
    API_URL="https://api.github.com/repos/\$REPO/releases/latest"
fi
RELEASE_JSON=\$(curl -fsSL "\$API_URL")
TAG=\$(echo "\$RELEASE_JSON" | grep '"tag_name"' | head -1 | sed 's/.*: "//;s/".*//')
CURRENT=\$(cat "$BIN_DIR/.version" 2>/dev/null || echo "none")
if [ "\$TAG" = "\$CURRENT" ]; then
    echo "already at \$TAG"
    exit 0
fi
echo "updating to \$TAG..."
curl -fsSL -L "https://github.com/\$REPO/releases/download/\$TAG/bolly-server-\$TARGET" -o "\$BIN.tmp" 2>/dev/null || \
    curl -fsSL -L "https://github.com/\$REPO/releases/download/\$TAG/bolly-\$TARGET" -o "\$BIN.tmp"
chmod +x "\$BIN.tmp"
mv "\$BIN.tmp" "\$BIN"
echo "\$TAG" > "$BIN_DIR/.version"
echo "updated to \$TAG — restart bolly to apply"
UPDATESCRIPT
chmod +x "$BIN_DIR/update"

# ─── Platform-specific service ────────────────────────────────────────────────
step "setting up service"

if [ "$PLATFORM" = "linux" ]; then
    # ── systemd ──
    if command -v systemctl &>/dev/null && [ "$(id -u)" -eq 0 ]; then
        SERVICE_FILE="/etc/systemd/system/bolly.service"
        cat > "$SERVICE_FILE" <<EOF
[Unit]
Description=Bolly AI Companion
After=network.target

[Service]
Type=simple
User=$(whoami)
WorkingDirectory=$BOLLY_DIR
Environment=BOLLY_HOME=$BOLLY_DIR
Environment=RUST_LOG=info
ExecStart=$BIN
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
EOF
        systemctl daemon-reload
        systemctl enable bolly >/dev/null 2>&1
        log "systemd service created"
        info "start:   sudo systemctl start bolly"
        info "logs:    sudo journalctl -u bolly -f"
    elif command -v systemctl &>/dev/null; then
        # User-level systemd (no root)
        SYSTEMD_DIR="$HOME/.config/systemd/user"
        mkdir -p "$SYSTEMD_DIR"
        cat > "$SYSTEMD_DIR/bolly.service" <<EOF
[Unit]
Description=Bolly AI Companion
After=network.target

[Service]
Type=simple
WorkingDirectory=$BOLLY_DIR
Environment=BOLLY_HOME=$BOLLY_DIR
Environment=RUST_LOG=info
ExecStart=$BIN
Restart=always
RestartSec=3

[Install]
WantedBy=default.target
EOF
        systemctl --user daemon-reload
        systemctl --user enable bolly >/dev/null 2>&1
        log "user systemd service created"
        info "start:   systemctl --user start bolly"
        info "logs:    journalctl --user -u bolly -f"
    else
        log "no systemd found — run manually: $BIN"
    fi

elif [ "$PLATFORM" = "macos" ]; then
    # ── launchd ──
    PLIST_DIR="$HOME/Library/LaunchAgents"
    PLIST="$PLIST_DIR/dev.bollyai.bolly.plist"
    mkdir -p "$PLIST_DIR"
    cat > "$PLIST" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>dev.bollyai.bolly</string>
    <key>ProgramArguments</key>
    <array>
        <string>$BIN</string>
    </array>
    <key>WorkingDirectory</key>
    <string>$BOLLY_DIR</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>BOLLY_HOME</key>
        <string>$BOLLY_DIR</string>
        <key>RUST_LOG</key>
        <string>info</string>
    </dict>
    <key>KeepAlive</key>
    <true/>
    <key>RunAtLoad</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$BOLLY_DIR/bolly.log</string>
    <key>StandardErrorPath</key>
    <string>$BOLLY_DIR/bolly.log</string>
</dict>
</plist>
EOF
    log "launchd service created"
    info "start:   launchctl load $PLIST"
    info "stop:    launchctl unload $PLIST"
    info "logs:    tail -f $BOLLY_DIR/bolly.log"
fi

# ─── Add to PATH ──────────────────────────────────────────────────────────────
SHELL_NAME=$(basename "$SHELL" 2>/dev/null || echo "bash")
RC_FILE="$HOME/.${SHELL_NAME}rc"

if ! echo "$PATH" | grep -q "$BIN_DIR"; then
    EXPORT_LINE="export PATH=\"$BIN_DIR:\$PATH\""
    if [ -f "$RC_FILE" ] && ! grep -q "$BIN_DIR" "$RC_FILE"; then
        echo "" >> "$RC_FILE"
        echo "# bolly" >> "$RC_FILE"
        echo "$EXPORT_LINE" >> "$RC_FILE"
        log "added to PATH in $RC_FILE"
    fi
fi

# ─── Doctor: stop old instance, fix config, restart ──────────────────────────
step "starting bolly"

export PATH="$BIN_DIR:$PATH"

# Read port from config (default 26559)
BOLLY_PORT=26559
if [ -f "$BOLLY_DIR/config.toml" ]; then
    CFG_PORT=$(grep -E '^port\s*=' "$BOLLY_DIR/config.toml" | head -1 | sed 's/.*=\s*//' | tr -d ' ')
    if [ -n "$CFG_PORT" ]; then
        BOLLY_PORT="$CFG_PORT"
    fi
fi
BOLLY_URL="http://localhost:$BOLLY_PORT"

# Stop any running bolly instance
OLD_PID=$(pgrep -f "$BIN_DIR/bolly" 2>/dev/null | head -1)
if [ -n "$OLD_PID" ]; then
    info "stopping old instance (PID: $OLD_PID)..."
    kill "$OLD_PID" 2>/dev/null
    sleep 1
    kill -9 "$OLD_PID" 2>/dev/null || true
    log "stopped old instance"
fi

# Unload launchd/systemd if loaded (will re-create below)
if [ "$PLATFORM" = "macos" ]; then
    PLIST="$HOME/Library/LaunchAgents/dev.bollyai.bolly.plist"
    launchctl unload "$PLIST" 2>/dev/null || true
elif command -v systemctl &>/dev/null; then
    systemctl --user stop bolly 2>/dev/null || true
fi

# Start fresh
"$BIN" &>/dev/null &
BOLLY_PID=$!

info "waiting for bolly to start..."
for i in $(seq 1 30); do
    if curl -sf "$BOLLY_URL/api/health" >/dev/null 2>&1; then
        break
    fi
    sleep 0.5
done

if curl -sf "$BOLLY_URL/api/health" >/dev/null 2>&1; then
    log "bolly is running on port $BOLLY_PORT"

    # Open browser
    if [ "$PLATFORM" = "macos" ]; then
        open "$BOLLY_URL" 2>/dev/null
    elif command -v xdg-open &>/dev/null; then
        xdg-open "$BOLLY_URL" 2>/dev/null
    fi

    echo ""
    echo -e "${BOLD}  ┌─────────────────────────────┐${NC}"
    echo -e "${BOLD}  │${NC}  ${GREEN}bolly is ready!${NC}            ${BOLD}│${NC}"
    echo -e "${BOLD}  └─────────────────────────────┘${NC}"
    echo ""
    echo -e "  ${CYAN}${BOLLY_URL}${NC} is open in your browser"
    echo ""
else
    warn "bolly didn't start automatically"
    echo ""
    echo -e "${BOLD}  ┌─────────────────────────────┐${NC}"
    echo -e "${BOLD}  │${NC}  ${GREEN}installation complete!${NC}     ${BOLD}│${NC}"
    echo -e "${BOLD}  └─────────────────────────────┘${NC}"
    echo ""
    echo -e "  ${YELLOW}→${NC} Run ${BOLD}bolly${NC} and visit ${CYAN}${BOLLY_URL}${NC}"
    echo ""
fi
