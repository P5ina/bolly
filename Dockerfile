# Runtime-only image for Fly.io — binary downloaded from GitHub releases on startup
FROM ubuntu:24.04

# Install all runtime dependencies
RUN apt-get update -qq && \
    apt-get install -y --no-install-recommends \
      ca-certificates curl sudo procps git jq ffmpeg \
      python3 python3-pip python3-venv \
      fontconfig fonts-liberation fonts-dejavu-core && \
    # Node.js
    curl -fsSL https://deb.nodesource.com/setup_20.x | bash - > /dev/null 2>&1 && \
    apt-get install -y nodejs > /dev/null 2>&1 && \
    npm install -g pnpm > /dev/null 2>&1 && \
    # yt-dlp
    curl -fsSL https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp && chmod +x /usr/local/bin/yt-dlp && \
    # cloudflared
    DARCH=$(dpkg --print-architecture) && \
    curl -fsSL "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-${DARCH}.deb" -o /tmp/cf.deb && \
    dpkg -i /tmp/cf.deb && rm /tmp/cf.deb && \
    # gh CLI
    curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg 2>/dev/null && \
    echo "deb [arch=${DARCH} signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" > /etc/apt/sources.list.d/github-cli.list && \
    apt-get update -qq && apt-get install -y gh && \
    # Playwright
    npx playwright@1.52.0 install --with-deps chromium && \
    # Cleanup
    rm -rf /var/lib/apt/lists/* /root/.cache/ms-playwright/.links /tmp/*

# Copy scripts only — no binary, no client build
COPY server/scripts/ /opt/bolly/scripts/
RUN cd /opt/bolly/scripts && npm install --omit=dev 2>/dev/null || true

ENV BOLLY_HOME=/data
ENV RUST_LOG=info,rig=warn
ENV BOLLY_SCRIPTS_DIR=/opt/bolly/scripts
ENV PLAYWRIGHT_BROWSERS_PATH=/data/.playwright

EXPOSE 8080
VOLUME /data

CMD ["/opt/bolly/scripts/entrypoint.sh"]
