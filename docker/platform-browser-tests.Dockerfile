# Disposable test runtime only. Build with --pull=false and a Dockerfile-only context.
FROM ubuntu:24.04

# Refuse an unexpected base account rather than editing, deleting or replacing it.
RUN test "$(getent passwd 1000 | cut -d: -f1)" = ubuntu \
    && test "$(getent passwd ubuntu | cut -d: -f6)" = /home/ubuntu \
    && mkdir -p /etc/postgresql-common \
    && printf 'create_main_cluster = false\n' > /etc/postgresql-common/createcluster.conf \
    && printf '#!/bin/sh\nexit 101\n' > /usr/sbin/policy-rc.d \
    && chmod 755 /usr/sbin/policy-rc.d \
    && apt-get -o Acquire::Retries=0 -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30 \
       -o APT::Update::Error-Mode=any update \
    && DEBIAN_FRONTEND=noninteractive apt-get -o Acquire::Retries=0 \
       -o Acquire::http::Timeout=30 -o Acquire::https::Timeout=30 install -y --no-install-recommends \
       bash ca-certificates coreutils findutils grep openssl postgresql-16 postgresql-client-16 \
       libnss3-tools libasound2t64 libatk-bridge2.0-0t64 libatk1.0-0t64 libatspi2.0-0t64 \
       libcairo2 libcups2t64 libdbus-1-3 libexpat1 libgbm1 libglib2.0-0t64 libgtk-3-0t64 \
       libnspr4 libnss3 libpango-1.0-0 libudev1 libvulkan1 libx11-6 libxcb1 libxcomposite1 \
       libxdamage1 libxext6 libxfixes3 libxkbcommon0 libxrandr2 libssl3t64 libgcc-s1 zlib1g libzstd1 \
    && apt-get clean \
    && rm -rf /var/lib/apt/lists/*

# Docker supplies this real image account's default HOME; never override host HOME.
USER ubuntu:ubuntu
WORKDIR /home/ubuntu
