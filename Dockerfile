FROM rust:latest AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y \
    pkg-config \
    libgtk-4-dev \
    libgraphene-1.0-dev \
    libpango1.0-dev \
    libgdk-pixbuf-2.0-dev \
    libatk1.0-dev \
    libglib2.0-dev \
    libssl-dev \
    ca-certificates \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release --bin redis_server

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    libgtk-4-dev \
    libglib2.0-dev \
    libpango1.0-dev \
    libgdk-pixbuf-2.0-dev \
    libatk1.0-dev \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/redis_server /usr/local/bin/redis_server

ENTRYPOINT ["redis_server"]
