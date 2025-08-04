FROM rust:latest AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY . .

RUN rustup target add x86_64-unknown-linux-musl

RUN cargo build --release --bin microservice --target x86_64-unknown-linux-musl

FROM alpine:latest

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/microservice /app/microservice_bin

RUN mkdir -p /app/logs

VOLUME ["/app/logs"]

# Entrypoint
CMD ["/app/microservice_bin"]
