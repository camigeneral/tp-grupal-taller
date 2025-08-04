FROM rust:latest AS builder

WORKDIR /app
COPY . .

RUN apt-get update && apt-get install -y pkg-config libssl-dev

RUN cargo build --release --bin llm_microservice

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Crear directorio para logs
RUN mkdir -p /app/logs

COPY --from=builder /app/target/release/llm_microservice /app/llm_microservice_bin

VOLUME ["/app/logs"]

CMD ["/app/llm_microservice_bin"]