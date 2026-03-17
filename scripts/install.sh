#!/bin/bash
# Bolly installer — sets up everything on a fresh Linux machine.
# Usage: curl -fsSL https://raw.githubusercontent.com/triangle-int/bolly/main/scripts/install.sh | bash
set -e

REPO="triangle-int/bolly"
INSTALL_DIR="/opt/bolly"
DATA_DIR="/data"
BIN="$INSTALL_DIR/bin/bolly"
USER="bolly"
SERVICE="bolly"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
DIM='\033[0;90m'
NC='\033[0m'

log()  { echo -e "${GREEN}[bolly]${NC} $1"; }
dim()  { echo -e "${DIM}$1${NC}"; }
fail() { echo -e "${RED}[error]${NC} $1"; exit 1; }

# --- Check root ---
if [ "$(id -u)" -ne 0 ]; then
    fail "run as root: sudo bash install.sh"
fi

# --- Detect arch ---
ARCH=$(uname -m)
case "$ARCH" in
    x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
    aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
    *) fail "unsupported architecture: $ARCH" ;;
esac

# --- Detect channel ---
CHANNEL="${BOLLY_CHANNEL:-stable}"
if [ "$CHANNEL" = "nightly" ]; then
    API_URL="https://api.github.com/repos/$REPO/releases/tags/nightly"
else
    API_URL="https://api.github.com/repos/$REPO/releases/latest"
fi

log "installing bolly ($CHANNEL) for $TARGET"

# --- System dependencies ---
log "installing system packages..."
apt-get update -qq
apt-get install -y --no-install-recommends \
    ca-certificates curl sudo procps git jq ffmpeg \
    python3 python3-pip python3-venv \
    fontconfig fonts-liberation fonts-dejavu-core \
    > /dev/null 2>&1

# --- Node.js ---
if ! command -v node &>/dev/null; then
    log "installing Node.js..."
    curl -fsSL https://deb.nodesource.com/setup_20.x | bash - > /dev/null 2>&1
    apt-get install -y nodejs > /dev/null 2>&1
    npm install -g pnpm > /dev/null 2>&1
fi

# --- yt-dlp ---
if ! command -v yt-dlp &>/dev/null; then
    log "installing yt-dlp..."
    curl -fsSL https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp
    chmod +x /usr/local/bin/yt-dlp
fi

# --- cloudflared ---
if ! command -v cloudflared &>/dev/null; then
    log "installing cloudflared..."
    DARCH=$(dpkg --print-architecture 2>/dev/null || echo amd64)
    curl -fsSL "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-${DARCH}.deb" -o /tmp/cloudflared.deb
    dpkg -i /tmp/cloudflared.deb > /dev/null 2>&1
    rm /tmp/cloudflared.deb
fi

# --- gh CLI ---
if ! command -v gh &>/dev/null; then
    log "installing GitHub CLI..."
    DARCH=$(dpkg --print-architecture 2>/dev/null || echo amd64)
    curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg 2>/dev/null
    echo "deb [arch=${DARCH} signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" > /etc/apt/sources.list.d/github-cli.list
    apt-get update -qq && apt-get install -y gh > /dev/null 2>&1
fi

# --- Chromium ---
if ! command -v chromium-browser &>/dev/null && ! command -v chromium &>/dev/null; then
    log "installing Chromium..."
    apt-get install -y chromium-browser > /dev/null 2>&1 || \
    apt-get install -y chromium > /dev/null 2>&1 || true
fi

# --- Create dirs ---
mkdir -p "$INSTALL_DIR/bin" "$INSTALL_DIR/scripts" "$DATA_DIR"

# --- Download binary ---
log "downloading bolly binary..."
RELEASE_JSON=$(curl -fsSL "$API_URL")
TAG=$(echo "$RELEASE_JSON" | jq -r '.tag_name')

if [ -z "$TAG" ] || [ "$TAG" = "null" ]; then
    fail "could not find release"
fi

ASSET_URL="https://github.com/$REPO/releases/download/$TAG/bolly-$TARGET"
curl -fsSL -L "$ASSET_URL" -o "$BIN"
chmod +x "$BIN"
echo "$TAG" > "$INSTALL_DIR/bin/.version"
log "downloaded $TAG"

# --- Create user ---
if ! id "$USER" &>/dev/null; then
    useradd -r -s /bin/bash -d "$DATA_DIR" "$USER"
fi
chown -R "$USER:$USER" "$DATA_DIR"

# --- Systemd service ---
log "creating systemd service..."
cat > /etc/systemd/system/$SERVICE.service <<EOF
[Unit]
Description=Bolly AI Companion
After=network.target

[Service]
Type=simple
User=$USER
WorkingDirectory=$DATA_DIR
Environment=BOLLY_HOME=$DATA_DIR
Environment=RUST_LOG=info,rig=warn
Environment=CHROMIUM_PATH=/usr/bin/chromium-browser
Environment=BOLLY_CHANNEL=$CHANNEL
ExecStart=$BIN
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable $SERVICE

# --- Update script ---
cat > "$INSTALL_DIR/bin/update" <<'UPDATESCRIPT'
#!/bin/bash
set -e
REPO="triangle-int/bolly"
BIN="/opt/bolly/bin/bolly"
CHANNEL="${BOLLY_CHANNEL:-stable}"
ARCH=$(uname -m)
case "$ARCH" in
    x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
    aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
esac
if [ "$CHANNEL" = "nightly" ]; then
    API_URL="https://api.github.com/repos/$REPO/releases/tags/nightly"
else
    API_URL="https://api.github.com/repos/$REPO/releases/latest"
fi
TAG=$(curl -fsSL "$API_URL" | jq -r '.tag_name')
CURRENT=$(cat /opt/bolly/bin/.version 2>/dev/null || echo "none")
if [ "$TAG" = "$CURRENT" ]; then
    echo "already at $TAG"
    exit 0
fi
echo "updating to $TAG..."
curl -fsSL -L "https://github.com/$REPO/releases/download/$TAG/bolly-$TARGET" -o "$BIN.tmp"
chmod +x "$BIN.tmp"
mv "$BIN.tmp" "$BIN"
echo "$TAG" > /opt/bolly/bin/.version
systemctl restart bolly
echo "updated to $TAG and restarted"
UPDATESCRIPT
chmod +x "$INSTALL_DIR/bin/update"

# --- Done ---
log "installation complete!"
echo ""
dim "  binary:  $BIN"
dim "  data:    $DATA_DIR"
dim "  service: systemctl start $SERVICE"
dim "  update:  /opt/bolly/bin/update"
dim "  config:  $DATA_DIR/config.toml"
echo ""
log "start with: systemctl start $SERVICE"
log "update with: /opt/bolly/bin/update"
