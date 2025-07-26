# FROM rust:latest AS builder

# WORKDIR /app
# COPY . .

# RUN rustup target add x86_64-unknown-linux-musl

# RUN cargo build --release -p microservice

# FROM alpine:latest

# RUN apk add --no-cache ca-certificates

# COPY --from=builder /app/target/release/microservice /usr/local/bin/microservice_bin
# CMD ["microservice_bin"]

FROM rust:latest AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY redis_server/ redis_server/
COPY microservice/ microservice/
COPY client/ client/
COPY rusty_docs/ rusty_docs/

RUN rustup target add x86_64-unknown-linux-musl

RUN cargo build --release --bin microservice --target x86_64-unknown-linux-musl

FROM alpine:latest

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/microservice /usr/local/bin/microservice_bin

CMD ["microservice_bin"]
