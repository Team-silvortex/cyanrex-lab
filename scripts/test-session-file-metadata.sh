#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable PostgreSQL only. No live installer or ambient database fallback.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
session_file_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$session_file_repo_root"

for session_file_pg_case in \
  baseline::postgres_session_upload_reservation_commits_and_gets_context_owner \
  cases::behavior::postgres_session_owner_metadata_is_private_and_uses_only_source_transactions \
  cases::lifecycle::postgres_session_metadata_reauthorizes_logout_expiry_membership_and_recreated_accounts \
  cases::policy::postgres_session_metadata_requires_current_audited_active_human_membership \
  cases::config::postgres_session_metadata_rejects_namespace_installation_limits_and_relation_drift \
  cases::records::postgres_session_metadata_validates_selected_records_and_reservation_full_ledger \
  cases::relations::postgres_original_source_and_registry_relation_replacement_rolls_back_reservation \
  cases::faults::postgres_session_upload_trigger_drift_and_deferred_faults_roll_back_exactly \
  cases::waits::postgres_session_metadata_row_and_marker_waits_recheck_freshness_expiry_and_cancel \
  cases::concurrency::postgres_session_upload_concurrent_quota_reservations_use_fresh_source_transactions; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_file_metadata_tdd \
    "$session_file_pg_case" -- --ignored --list | grep -Fx "$session_file_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_file_metadata_tdd \
    "$session_file_pg_case" -- --ignored --exact --nocapture
done
