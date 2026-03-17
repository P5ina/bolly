# Stage 1: Build client
FROM node:20-alpine AS client-build
RUN corepack enable
WORKDIR /app/client
COPY client/package.json client/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY client/ .
RUN pnpm build

# Stage 2: Prepare Rust dependency cache
FROM rust:bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY server/ server/
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Build dependencies (cached layer)
FROM chef AS deps
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json -p server

# Stage 4: Build server with embedded client
FROM deps AS server-build
COPY Cargo.toml Cargo.lock ./
COPY server/ server/
COPY --from=client-build /app/client/build client/build/
ARG GIT_HASH=dev
RUN GIT_HASH=${GIT_HASH} cargo build --release -p server

# Stage 5: Runtime — minimal base, install.sh does the rest
FROM debian:bookworm-slim

# Copy binary and scripts
COPY --from=server-build /app/target/release/server /opt/bolly/bin/bolly
COPY scripts/install.sh /opt/bolly/install.sh
COPY server/scripts/ /opt/bolly/scripts/

# Install all deps using the same script as bare-metal installs
# Skip binary download (already have it) and systemd setup (Docker manages process)
RUN apt-get update -qq && \
    apt-get install -y --no-install-recommends ca-certificates curl jq sudo && \
    bash -c '\
      set -e; \
      apt-get install -y --no-install-recommends \
        procps git ffmpeg \
        python3 python3-pip python3-venv \
        fontconfig fonts-liberation fonts-dejavu-core && \
      # Node.js \
      curl -fsSL https://deb.nodesource.com/setup_20.x | bash - > /dev/null 2>&1 && \
      apt-get install -y nodejs > /dev/null 2>&1 && \
      npm install -g pnpm > /dev/null 2>&1 && \
      # yt-dlp \
      curl -fsSL https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp && chmod +x /usr/local/bin/yt-dlp && \
      # cloudflared \
      DARCH=$(dpkg --print-architecture) && \
      curl -fsSL "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-${DARCH}.deb" -o /tmp/cf.deb && \
      dpkg -i /tmp/cf.deb && rm /tmp/cf.deb && \
      # gh CLI \
      curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg 2>/dev/null && \
      echo "deb [arch=${DARCH} signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" > /etc/apt/sources.list.d/github-cli.list && \
      apt-get update -qq && apt-get install -y gh && \
      # Playwright \
      npx playwright@1.52.0 install --with-deps chromium && \
      # Browse tool deps \
      cd /opt/bolly/scripts && npm install --omit=dev && \
      # Cleanup \
      rm -rf /var/lib/apt/lists/* /root/.cache/ms-playwright/.links /tmp/* \
    '

ENV BOLLY_HOME=/data
ENV RUST_LOG=info,rig=warn
ENV BOLLY_SCRIPTS_DIR=/opt/bolly/scripts
ENV PLAYWRIGHT_BROWSERS_PATH=/data/.playwright

EXPOSE 8080
VOLUME /data

CMD ["/opt/bolly/scripts/entrypoint.sh"]
