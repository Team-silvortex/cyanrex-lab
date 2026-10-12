#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable PostgreSQL only; no live installer or ambient database fallback.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
session_publisher_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$session_publisher_repo_root"

for session_publisher_pg_case in \
  baseline::postgres_session_publisher_grant_reads_exact_current_subject_metadata \
  behavior::postgres_session_publisher_grant_own_metadata_is_not_role_or_current_permission \
  config::postgres_session_publisher_grant_rejects_scope_installation_limits_and_shape_drift \
  faults::postgres_session_publisher_grant_selected_corruption_fails_but_foreign_records_stay_hidden \
  lifecycle::postgres_session_publisher_grant_rechecks_current_membership_session_and_account_generation \
  transactions::postgres_session_publisher_grant_fresh_reads_override_rr_but_preserve_readonly_settings \
  waits::postgres_session_publisher_grant_shared_row_waits_recheck_expiry_cancel_and_prevent_history_append; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_file_publisher_grant_tdd \
    "$session_publisher_pg_case" -- --ignored --list | grep -Fx "$session_publisher_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_file_publisher_grant_tdd \
    "$session_publisher_pg_case" -- --ignored --exact --nocapture
done
