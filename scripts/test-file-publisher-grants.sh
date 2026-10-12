#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable PostgreSQL only; no live installer or ambient database fallback.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
publisher_grant_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$publisher_grant_repo_root"

for publisher_grant_pg_case in \
  baseline::postgres_fresh_publisher_grant_issue_persists_and_reopens \
  cases::behavior::postgres_publisher_grants_filter_exact_subject_and_account_generation \
  cases::revocation::postgres_publisher_grant_revocation_is_one_cas_with_immutable_issuer_and_actor_history \
  cases::quota::postgres_expired_revoked_and_cross_generation_grants_retain_global_quota \
  cases::concurrency::postgres_queued_rr_publishers_use_fresh_ledger_and_only_one_wins_quota \
  cases::schema::postgres_publisher_grant_namespace_installation_scope_limits_and_shapes_are_not_adopted \
  cases::records::postgres_publisher_grant_head_and_ordered_history_corruption_fail_closed \
  cases::ledger::postgres_publisher_grant_foreign_corruption_and_excess_inventory_precede_conflict_and_quota \
  cases::faults::postgres_publisher_grant_suppressed_or_tampered_issue_and_revoke_roll_back \
  cases::deferred::postgres_publisher_grant_deferred_failures_original_relation_pins_and_path_drift_roll_back \
  cases::waits::postgres_cancelled_real_publisher_grant_deferred_flush_wait_retains_no_retry_authority \
  cases::shadow::postgres_publisher_grant_shadow_namespaces_and_marker_wait_drift_are_rejected; do
  cargo test --manifest-path engine/Cargo.toml --locked --test file_publisher_grant_tdd \
    "$publisher_grant_pg_case" -- --ignored --list | grep -Fx "$publisher_grant_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test file_publisher_grant_tdd \
    "$publisher_grant_pg_case" -- --ignored --exact --nocapture
done
