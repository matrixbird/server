FROM rust:latest AS builder

WORKDIR /app
COPY . .

RUN cargo build 

FROM debian:bookworm-slim
RUN apt-get update && apt install -y openssl

WORKDIR /app

COPY --from=builder /app/target/debug/public-appservice /app/public-appservice

EXPOSE 8989

ENTRYPOINT ["/app/public-appservice"]
