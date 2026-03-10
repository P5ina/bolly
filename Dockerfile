# Stage 1: Build client
FROM node:20-alpine AS client-build
RUN corepack enable
WORKDIR /app/client
COPY client/package.json client/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY client/ .
RUN pnpm build

# Stage 2: Build server
FROM rust:bookworm AS server-build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY server/ server/
RUN cargo build --release -p server

# Stage 3: Runtime
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

CMD ["bolly"]
