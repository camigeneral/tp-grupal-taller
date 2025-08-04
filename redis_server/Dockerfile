FROM rust:latest AS builder

WORKDIR /app
COPY . . 

RUN rustup target add x86_64-unknown-linux-musl

RUN cargo build --release --bin redis_server --target x86_64-unknown-linux-musl

FROM alpine:latest

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/redis_server /app/redis_server_bin

COPY redis_server/conf_files /app/redis_server/conf_files
COPY redis_server/rdb_files /app/redis_server/rdb_files

RUN mkdir -p /app/logs
RUN chmod +x /app/redis_server_bin

VOLUME ["/app/logs"]

# Entrypoint
CMD ["/app/redis_server_bin", "4000"]
