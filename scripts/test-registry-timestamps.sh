#!/usr/bin/env bash
set -euo pipefail

# Focused synthetic corruption regressions only, never an automatic migration or live-data audit.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
if (( $# != 0 )); then
  printf '%s\n' 'This fixed regression runner does not accept arguments.' >&2
  exit 2
fi
registry_timestamps_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$registry_timestamps_repo_root"

for registry_timestamps_case in \
  'collaboration_identity_store_tdd|timestamps::postgres_identity_nullable_retirement_rejects_invalid_nonnull_times_without_reviving_accounts' \
  'collaboration_identity_store_tdd|timestamps::postgres_identity_retirement_trigger_invalid_time_rolls_back_binding_and_principal' \
  'collaboration_identity_audit_tdd|timestamps::postgres_lifecycle_timestamp_reads_cover_nullable_retirement_head_history_and_old_replay' \
  'collaboration_identity_audit_tdd|timestamps::postgres_lifecycle_audit_append_invalid_time_rolls_back_binding_and_retirement' \
  'collaboration_policy_audit_tdd|timestamps::postgres_policy_audit_head_rejects_invalid_times_and_preserves_finite_boundaries' \
  'collaboration_policy_audit_tdd|timestamps::postgres_policy_audit_old_receipt_times_fail_history_and_replay_with_healthy_head' \
  'collaboration_policy_audit_tdd|timestamps::postgres_policy_audit_invalid_inserted_time_rolls_back_membership_grant_and_receipt' \
  'durable_reconciliation_tdd|registry_timestamps::postgres_reconcile_invalid_registry_times_reject_their_exact_stage_without_writing' \
  'durable_reconciliation_tdd|registry_timestamps::postgres_reconcile_finite_audit_times_preserve_sequence_based_history' \
  'durable_reconciliation_tdd|registry_timestamps::postgres_session_command_bad_registry_times_cannot_authorize_a_binding'; do
  IFS='|' read -r registry_timestamps_target registry_timestamps_name <<< "$registry_timestamps_case"
  cargo test --manifest-path engine/Cargo.toml --locked --test "$registry_timestamps_target" "$registry_timestamps_name" -- --ignored --list | grep -Fx "$registry_timestamps_name: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test "$registry_timestamps_target" "$registry_timestamps_name" -- --ignored --exact --nocapture
done
