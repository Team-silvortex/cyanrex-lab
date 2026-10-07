#!/usr/bin/env bash
set -euo pipefail

# Explicit prepared-source cleanup on synthetic records; no live database or automatic scheduler.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
session_cleanup_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$session_cleanup_repo_root"
session_cleanup_prefix=services::auth_service::durable_source::session_cleanup::sql_tests

for session_cleanup_case in \
  postgres_session_cleanup_prunes_at_most_128_and_preserves_active_sessions_and_accounts \
  behavior::postgres_session_cleanup_is_source_wide_but_preserves_other_sources_and_live_rows \
  behavior::postgres_session_cleanup_independent_pools_confirm_disjoint_bounded_batches \
  behavior::postgres_session_cleanup_uses_fresh_database_cutoff_after_its_source_writer_wait \
  behavior::postgres_session_cleanup_rejects_corrupt_selected_records_and_source_errors_without_repair \
  faults::postgres_session_cleanup_rejects_suppressed_deletion_and_future_reinsertion \
  faults::postgres_session_cleanup_rolls_back_source_drift_and_selected_row_trigger_changes \
  faults::postgres_session_cleanup_deferred_commit_failure_returns_no_confirmed_count \
  faults::postgres_session_cleanup_cancellation_during_post_delete_wait_is_observed_rolled_back; do
  session_cleanup_name="$session_cleanup_prefix::$session_cleanup_case"
  cargo test --manifest-path engine/Cargo.toml --locked --lib "$session_cleanup_name" -- --ignored --list | grep -Fx "$session_cleanup_name: test"
  cargo test --manifest-path engine/Cargo.toml --locked --lib "$session_cleanup_name" -- --ignored --exact --nocapture
done
