FROM rust:latest AS builder

WORKDIR /app

COPY llm_microservice/Cargo.toml ./llm_microservice/Cargo.toml
COPY llm_microservice/src/main.rs ./llm_microservice/src/main.rs

COPY rusty_docs/Cargo.toml ./rusty_docs/Cargo.toml
COPY rusty_docs/src/lib.rs ./rusty_docs/src/lib.rs


WORKDIR /app/llm_microservice
RUN cargo fetch 

# Copiar todo el código para build completo
WORKDIR /app
COPY llm_microservice/ ./llm_microservice/
COPY rusty_docs/ ./rusty_docs/

RUN apt-get update && apt-get install -y pkg-config libssl-dev

WORKDIR /app/llm_microservice

RUN touch src/main.rs

RUN cargo build --release --bin llm_microservice

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

RUN mkdir -p /app/logs

COPY --from=builder /app/llm_microservice/target/release/llm_microservice /app/llm_microservice_bin

VOLUME ["/app/logs"]

EXPOSE 4030

CMD ["/app/llm_microservice_bin"]
