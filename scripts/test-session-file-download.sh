#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable PostgreSQL only; a manifest is not byte admission or delivery.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
download_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$download_repo_root"

for download_pg_case in \
  baseline::postgres_current_recipient_observes_exact_published_manifest_after_publisher_revocation \
  selectors::postgres_download_exact_selectors_filter_same_publisher_foreign_corruption_before_decode \
  grants::postgres_download_requires_exact_active_recipient_grant_without_role_or_publisher_bypass \
  lifecycle::postgres_download_requires_current_audited_human_and_exact_recipient_generation \
  configuration::postgres_download_uses_source_transaction_and_rejects_pinned_configuration_drift \
  records::postgres_download_rejects_selected_recipient_history_and_hidden_publication_corruption \
  relations::postgres_download_retains_source_registry_and_target_relation_fences \
  fences::postgres_download_fences_queued_revocation_and_history_append_until_final_commit \
  waits::postgres_download_final_clock_rechecks_session_and_grant_windows_after_real_wait \
  cancellation::postgres_download_cancelled_real_wait_drains_without_metadata_or_file_writes; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_file_download_tdd \
    "$download_pg_case" -- --ignored --list | grep -Fx "$download_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_file_download_tdd \
    "$download_pg_case" -- --ignored --exact --nocapture
done
