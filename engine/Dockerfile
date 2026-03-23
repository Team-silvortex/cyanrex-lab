ARG RUST_IMAGE=rust:bookworm
ARG DEBIAN_IMAGE=debian:bookworm
FROM ${RUST_IMAGE} AS builder
WORKDIR /app
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
    CARGO_NET_RETRY=10

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

FROM ${DEBIAN_IMAGE} AS runtime
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget clang bpftool linux-libc-dev libbpf-dev \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/cyanrex-engine /usr/local/bin/cyanrex-engine

ENV ENGINE_HOST=0.0.0.0
ENV ENGINE_PORT=8080
EXPOSE 8080

CMD ["cyanrex-engine"]
