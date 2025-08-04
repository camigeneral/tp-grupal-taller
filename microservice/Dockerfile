FROM rust:latest AS builder

WORKDIR /app

# Copiamos solo lo necesario para que no vuelva a compilar todo al mínimo cambio
COPY Cargo.toml Cargo.lock ./
COPY . .

RUN rustup target add x86_64-unknown-linux-musl

RUN cargo build --release --bin microservice --target x86_64-unknown-linux-musl

# Fase final
FROM alpine:latest

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/microservice /app/microservice_bin

RUN mkdir -p /app/logs

VOLUME ["/app/logs"]

# Entrypoint
CMD ["/app/microservice_bin"]
