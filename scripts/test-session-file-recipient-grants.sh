#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable PostgreSQL only; no ambient database or live installation.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
session_recipient_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$session_recipient_repo_root"

for session_recipient_pg_case in \
  baseline::postgres_session_recipient_grant_reads_only_exact_current_recipient_metadata \
  behavior::postgres_session_recipient_metadata_is_private_not_role_time_or_download_authority \
  config::postgres_session_recipient_metadata_rejects_source_target_scope_incarnation_and_schema_drift \
  isolation::postgres_session_recipient_metadata_uses_fresh_rc_without_overriding_readonly_or_pool_settings \
  lifecycle::postgres_session_recipient_metadata_requires_current_audited_human_and_account_generation \
  records::postgres_session_recipient_selected_canonical_and_history_faults_do_not_leak_foreign_records \
  waits::postgres_session_recipient_row_waits_recheck_expiry_cancel_and_fence_history_append; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_file_recipient_grant_tdd \
    "$session_recipient_pg_case" -- --ignored --list | grep -Fx "$session_recipient_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_file_recipient_grant_tdd \
    "$session_recipient_pg_case" -- --ignored --exact --nocapture
done
