FROM rust:1-slim-bookworm AS builder
WORKDIR /build

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY templates ./templates
COPY static ./static

RUN cargo build --release --locked || cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -m -u 1000 -s /bin/bash app \
    && mkdir -p /data \
    && chown app:app /data

COPY --from=builder /build/target/release/roulette /usr/local/bin/roulette

USER app
WORKDIR /home/app

ENV PORT=3000 \
    DB_PATH=/data/roulette.db \
    RUST_LOG=info,roulette=info \
    BROUTER_URL=https://brouter.de

EXPOSE 3000
VOLUME ["/data"]

CMD ["/usr/local/bin/roulette"]
