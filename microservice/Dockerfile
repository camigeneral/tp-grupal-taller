FROM rust:1.75-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    musl-tools \
    && rm -rf /var/lib/apt/lists/*
    
WORKDIR /app

COPY microservice/Cargo.toml ./microservice/Cargo.toml
COPY microservice/src/main.rs ./microservice/src/main.rs

COPY rusty_docs/Cargo.toml ./rusty_docs/Cargo.toml
COPY rusty_docs/src/lib.rs ./rusty_docs/src/lib.rs

RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /app/microservice
RUN cargo fetch --target x86_64-unknown-linux-musl

# Copiar el resto del código y carpeta shared
WORKDIR /app
COPY microservice/ ./microservice/
COPY rusty_docs/ ./rusty_docs/

WORKDIR /app/microservice

RUN touch src/main.rs

RUN cargo build --release --bin microservice --target x86_64-unknown-linux-musl --target-dir ../target \
    && strip ../target/x86_64-unknown-linux-musl/release/microservice

FROM alpine:latest

RUN apk add --no-cache ca-certificates     

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/microservice /app/microservice_bin

RUN mkdir -p /app/logs

VOLUME ["/app/logs"]

EXPOSE 5000

ENTRYPOINT ["/app/microservice_bin"]
