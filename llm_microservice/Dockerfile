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

COPY llm_microservice/ llm_microservice/

RUN rustup target add x86_64-unknown-linux-musl

RUN cargo build --release --bin llm_microservice --target x86_64-unknown-linux-musl

EXPOSE 4030

FROM alpine:latest

RUN apk add --no-cache ca-certificates

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/llm_microservice /usr/local/bin/llm_microservice_bin

CMD ["llm_microservice_bin"]
