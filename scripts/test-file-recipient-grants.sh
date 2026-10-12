#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable PostgreSQL only; no ambient database or live installation.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
recipient_grant_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$recipient_grant_repo_root"

for recipient_grant_pg_case in \
  baseline::postgres_recipient_grant_records_exact_file_and_independent_accounts \
  cases::behavior::postgres_recipient_grants_bind_exact_parties_file_and_independent_publisher_ledger \
  cases::concurrency::postgres_queued_rr_recipient_writers_use_fresh_retained_quota_and_preserve_readonly \
  cases::deferred::postgres_recipient_deferred_errors_original_pins_foreign_delta_and_sync_probe_are_exact \
  cases::faults::postgres_recipient_suppressed_or_tampered_issue_revoke_roll_back_exactly \
  cases::history::postgres_recipient_complete_history_foreign_ledger_and_excess_inventory_precede_conflict_quota \
  cases::quota::postgres_recipient_expired_revoked_and_account_generations_retain_global_quota \
  cases::records::postgres_recipient_canonical_head_full_file_and_each_account_pin_corruption_is_rejected \
  cases::revocation::postgres_recipient_revocation_is_single_cas_and_preserves_file_parties_issuer_history \
  cases::schema::postgres_recipient_namespace_scope_incarnation_limits_and_relation_shape_are_not_adopted \
  cases::shadow::postgres_recipient_grant_raw_path_shadow_and_fresh_marker_drift_are_rejected \
  cases::waits::postgres_cancelled_real_recipient_deferred_wait_observes_rollback_without_retry_authority; do
  cargo test --manifest-path engine/Cargo.toml --locked --test file_recipient_grant_tdd \
    "$recipient_grant_pg_case" -- --ignored --list | grep -Fx "$recipient_grant_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test file_recipient_grant_tdd \
    "$recipient_grant_pg_case" -- --ignored --exact --nocapture
done
