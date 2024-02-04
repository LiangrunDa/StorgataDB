FROM rust:latest as builder
WORKDIR /app
COPY . .
RUN cargo build --release --bin storgata-db

FROM debian:bullseye
RUN apt-get update && apt-get install -y dnsutils
WORKDIR /app
COPY --from=builder /app/target/release/storgata-db .
COPY --from=builder /app/run-kv.sh .
CMD ["./run-kv.sh"]
