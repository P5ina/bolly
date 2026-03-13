# Stage 0: Node binaries for runtime (glibc-based, not Alpine)
FROM node:20-slim AS node-bin

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

# Stage 4: Build server (only recompiles when src changes)
FROM deps AS server-build
COPY Cargo.toml Cargo.lock ./
COPY server/ server/
ARG GIT_HASH=dev
RUN echo "build: ${GIT_HASH}" > /tmp/build-info && \
    GIT_HASH=${GIT_HASH} cargo build --release -p server

# Stage 5: Runtime
FROM debian:bookworm-slim

# Copy Node.js + npm from glibc-based image (not Alpine/musl), install pnpm
COPY --from=node-bin /usr/local/bin/node /usr/local/bin/node
COPY --from=node-bin /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -sf ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm && \
    ln -sf ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx && \
    npm install -g pnpm

# Install system packages, fonts, Playwright Chromium in one apt session
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
      ca-certificates curl sudo procps \
      python3 python3-pip python3-venv \
      git jq \
      fontconfig \
      fonts-liberation fonts-dejavu-core fonts-noto-core fonts-noto-cjk \
      fonts-noto-color-emoji fonts-noto-mono \
      fonts-firacode fonts-open-sans fonts-roboto fonts-lato \
      fonts-inter fonts-font-awesome && \
    fc-cache -f && \
    npx playwright@1.52.0 install --with-deps chromium && \
    ARCH=$(dpkg --print-architecture) && \
    curl -fsSL https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-${ARCH}.deb -o /tmp/cloudflared.deb && \
    dpkg -i /tmp/cloudflared.deb && rm /tmp/cloudflared.deb && \
    rm -rf /var/lib/apt/lists/* /root/.cache/ms-playwright/.links

# Copy browse tool scripts and install deps
COPY server/scripts/ /opt/bolly/scripts/
RUN cd /opt/bolly/scripts && npm install --omit=dev

COPY --from=server-build /app/target/release/server /usr/local/bin/bolly
COPY --from=client-build /app/client/build /opt/bolly/static

ENV BOLLY_HOME=/data
ENV RUST_LOG=info,rig=warn
ENV BOLLY_SCRIPTS_DIR=/opt/bolly/scripts

EXPOSE 8080
VOLUME /data

CMD ["sh", "-c", "mkdir -p /data && grep -q static_dir /data/config.toml 2>/dev/null || printf 'static_dir = \"/opt/bolly/static\"\\n' >> /data/config.toml && exec bolly"]
