# syntax=docker/dockerfile:1.7

FROM rust:1.90-bookworm AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget clang bpftool linux-libc-dev libbpf-dev \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/cyanrex-engine /usr/local/bin/cyanrex-engine

ENV ENGINE_HOST=0.0.0.0
ENV ENGINE_PORT=8080
EXPOSE 8080

CMD ["cyanrex-engine"]
