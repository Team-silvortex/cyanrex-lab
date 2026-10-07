#!/usr/bin/env bash
set -euo pipefail

# Read-only corruption/privilege cases require an explicitly disposable PostgreSQL instance.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
reconcile_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$reconcile_repo_root"

for reconcile_pg_case in \
  postgres_reconcile_bootstrap_is_consistent_secret_free_and_read_only \
  postgres_reconcile_unbound_disabled_retired_and_expired_states_are_distinct \
  postgres_reconcile_deletion_recreation_and_password_rotation_preserve_history \
  postgres_reconcile_management_requires_source_binding_and_explicit_grant_not_roles \
  faults::postgres_reconcile_schema_scope_and_row_security_fail_closed_without_installing \
  faults::postgres_reconcile_missing_or_recreated_source_and_malformed_sessions_are_detected \
  faults::postgres_reconcile_identity_and_policy_heads_must_match_current_rows \
  faults::postgres_reconcile_checks_entire_history_not_only_the_latest_receipt \
  faults::postgres_reconcile_audit_payload_limits_and_missing_manager_never_report_success \
  faults::postgres_reconcile_keeps_one_mvcc_snapshot_across_interleaved_commits \
  faults::postgres_reconcile_timeout_and_cancellation_publish_no_partial_result_or_write \
  cli::postgres_reconcile_cli_requires_only_read_privileges_and_never_returns_enrollment \
  timestamps::postgres_reconcile_invalid_session_times_return_an_error_without_panicking_or_writing \
  timestamps::postgres_reconcile_finite_session_time_boundaries_preserve_expiry_counts \
  registry_timestamps::postgres_reconcile_invalid_registry_times_reject_their_exact_stage_without_writing \
  registry_timestamps::postgres_reconcile_finite_audit_times_preserve_sequence_based_history \
  registry_timestamps::postgres_session_command_bad_registry_times_cannot_authorize_a_binding; do
  cargo test --manifest-path engine/Cargo.toml --locked --test durable_reconciliation_tdd "$reconcile_pg_case" -- --ignored --list | grep -Fx "$reconcile_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test durable_reconciliation_tdd "$reconcile_pg_case" -- --ignored --exact --nocapture
done
