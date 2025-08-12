FROM rust:1.75-slim AS builder

# Instalar dependencias para musl
RUN apt-get update && apt-get install -y \
    pkg-config \
    musl-tools \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copiar solo los Cargo.toml y archivos mínimos para cachear dependencias
COPY redis_server/Cargo.toml ./redis_server/Cargo.toml
COPY redis_server/src/main.rs ./redis_server/src/main.rs

COPY rusty_docs/Cargo.toml ./rusty_docs/Cargo.toml
COPY rusty_docs/src/lib.rs ./rusty_docs/src/lib.rs

# Configurar target musl
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /app/redis_server
RUN cargo fetch --target x86_64-unknown-linux-musl

# Copiar el resto del código
WORKDIR /app
COPY redis_server/ ./redis_server/
COPY rusty_docs/ ./rusty_docs/

WORKDIR /app/redis_server

# Forzar rebuild del código fuente
RUN touch src/main.rs

RUN cargo build --release --bin redis_server --target x86_64-unknown-linux-musl --target-dir ../target \
    && strip ../target/x86_64-unknown-linux-musl/release/redis_server

# === RUNTIME STAGE ===
FROM alpine:latest

RUN apk add --no-cache ca-certificates 

# Copiar binario optimizado
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/redis_server /app/redis_server_bin

COPY redis_server/conf_files /app/redis_server/conf_files
COPY redis_server/rdb_files /app/redis_server/rdb_files


ENTRYPOINT ["/app/redis_server_bin"]