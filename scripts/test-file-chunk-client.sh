#!/usr/bin/env bash
set -euo pipefail

# Disposable SQL plus native loopback/TLS fixtures; never a normal-app listener or deployment.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
command -v openssl >/dev/null
chunk_client_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$chunk_client_repo_root"

for chunk_client_case in \
  source::postgres_client_fetches_actual_authorized_http_bytes_and_stages_without_server_mutations \
  tls::openssl_client_requires_trusted_ca_and_exact_ip_san_before_real_private_staging; do
  cargo test --manifest-path engine/Cargo.toml --locked --test file_chunk_client_tdd \
    "$chunk_client_case" -- --ignored --list | grep -Fx "$chunk_client_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test file_chunk_client_tdd \
    "$chunk_client_case" -- --ignored --exact --nocapture
done
