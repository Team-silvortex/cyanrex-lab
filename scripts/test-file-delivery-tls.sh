#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable native TLS verification, not a listener or deployed-LAN acceptance run.
command -v openssl >/dev/null
delivery_tls_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$delivery_tls_repo_root"

delivery_tls_cases=(
  acceptance::openssl_controlled_delivery_verifies_complete_large_empty_and_short_selected_outputs
  security::openssl_controlled_delivery_rejects_unknown_ca_wrong_ip_san_and_redirect_before_files
  cancellation::openssl_explicit_cancellation_at_tls_manifest_and_later_chunk_boundaries_preserves_only_original_ack_prefix
)
for delivery_tls_case in "${delivery_tls_cases[@]}"; do
  cargo test --manifest-path engine/Cargo.toml --locked --test file_delivery_tls_tdd \
    "$delivery_tls_case" -- --ignored --list | grep -Fx "$delivery_tls_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test file_delivery_tls_tdd \
    "$delivery_tls_case" -- --ignored --exact --nocapture
done
