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
RUN cargo build --release -p server

# Stage 5: Runtime
FROM debian:bookworm-slim
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates curl sudo && \
    rm -rf /var/lib/apt/lists/*

COPY --from=server-build /app/target/release/server /usr/local/bin/bolly
COPY --from=client-build /app/client/build /opt/bolly/static

ENV BOLLY_HOME=/data
ENV RUST_LOG=info

EXPOSE 8080
VOLUME /data

CMD ["sh", "-c", "mkdir -p /data && grep -q static_dir /data/config.toml 2>/dev/null || printf 'static_dir = \"/opt/bolly/static\"\\n' >> /data/config.toml && exec bolly"]
