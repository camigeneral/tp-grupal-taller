FROM rust:latest AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY redis_server/ redis_server/
COPY microservice/ microservice/
COPY client/ client/
COPY rusty_docs/ rusty_docs/
COPY redis_server/config_files/ redis_server/config_files/
COPY redis_server/rdb_files/ redis_server/rdb_files/


RUN cargo build --release --bin redis_server

FROM debian:bookworm-slim

COPY --from=builder /app/target/release/redis_server /usr/local/bin/redis_server

CMD ["redis_server", "4000"]
