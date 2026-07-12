ARG RUST_IMAGE=rust:bookworm
ARG DEBIAN_IMAGE=debian:bookworm
FROM ${RUST_IMAGE} AS builder
ARG CARGO_SOURCE_MIRROR=
WORKDIR /app
ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse \
    CARGO_NET_RETRY=10

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN if [ -n "${CARGO_SOURCE_MIRROR}" ]; then \
      mkdir -p .cargo; \
      printf '[source.crates-io]\nreplace-with = "mirror"\n[source.mirror]\nregistry = "%s"\n' \
        "${CARGO_SOURCE_MIRROR}" > .cargo/config.toml; \
    fi \
    && cargo build --release

FROM ${DEBIAN_IMAGE} AS runtime
ARG APT_MIRROR=deb.debian.org
WORKDIR /app

RUN sed -i "s|deb.debian.org|${APT_MIRROR}|g" /etc/apt/sources.list.d/debian.sources \
    && apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates wget clang bpftool linux-libc-dev libbpf-dev \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/cyanrex-engine /usr/local/bin/cyanrex-engine

ENV ENGINE_HOST=0.0.0.0
ENV ENGINE_PORT=8080
EXPOSE 8080

CMD ["cyanrex-engine"]
