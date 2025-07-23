FROM rust:latest AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY redis_server/ redis_server/
COPY microservice/ microservice/
COPY client/ client/
COPY rusty_docs/ rusty_docs/

RUN rustup target add x86_64-unknown-linux-musl

RUN cargo build --release --bin redis_server --target x86_64-unknown-linux-musl

FROM alpine:latest

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/redis_server /usr/local/bin/redis_server_bin

COPY redis_server/conf_files /usr/local/bin/redis_server/conf_files

CMD ["redis_server_bin", "4000"]
