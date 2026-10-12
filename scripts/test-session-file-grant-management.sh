#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable PostgreSQL only; never consult an ambient application database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
file_management_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$file_management_repo_root"

for file_management_pg_case in \
  baseline::postgres_session_explicit_role_free_manager_issues_with_derived_attribution \
  behavior::postgres_session_file_manager_crud_cas_derives_actor_and_preserves_original_issuer \
  authorization::postgres_session_file_manager_is_explicit_principal_generation_not_teacher_or_deployment_role \
  targets::postgres_session_file_issue_requires_current_audited_human_target_but_revoke_allows_retired_or_deleted_subject \
  actor_lifecycle::postgres_session_file_manager_rechecks_session_human_membership_audit_and_recreated_generation \
  schema::postgres_session_file_grant_management_rejects_namespace_installation_scope_shape_and_foreign_ledger_drift \
  faults::postgres_session_file_grant_immediate_and_deferred_source_subject_ledger_faults_roll_back \
  relations::postgres_session_file_grants_reject_original_source_registry_and_target_relation_replacement \
  waits::postgres_session_file_grant_real_waits_recheck_expiry_logout_fresh_marker_and_cancelled_writes \
  concurrency::postgres_session_file_grant_queued_rr_quota_and_revoke_cas_use_fresh_transactions_without_overriding_readonly; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_file_grant_management_tdd \
    "$file_management_pg_case" -- --ignored --list | grep -Fx "$file_management_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_file_grant_management_tdd \
    "$file_management_pg_case" -- --ignored --exact --nocapture
done
