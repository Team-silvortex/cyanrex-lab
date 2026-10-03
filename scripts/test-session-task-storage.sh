#!/usr/bin/env bash
set -euo pipefail

# Authorization and fault injection fixtures require an explicitly disposable database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
session_task_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$session_task_repo_root"

for session_task_pg_case in \
  postgres_session_tasks_owner_is_derived_without_teacher_or_deployment_grants \
  postgres_session_tasks_reject_unbound_missing_membership_and_revoked_sessions \
  postgres_session_tasks_reject_domain_or_artifact_work_until_evidence_is_authorized \
  postgres_session_tasks_revision_conflicts_and_terminal_states_preserve_events \
  postgres_session_tasks_retired_and_recreated_accounts_cannot_reuse_old_work \
  postgres_session_tasks_target_namespaces_must_exist_and_cannot_alias_source \
  faults::postgres_session_tasks_scope_and_missing_audits_cannot_authorize_writes \
  faults::postgres_session_tasks_suspended_members_and_archived_workspaces_are_denied \
  faults::postgres_session_tasks_event_and_deferred_commit_failures_publish_nothing \
  faults::postgres_session_tasks_post_write_session_or_membership_changes_roll_back \
  faults::postgres_session_tasks_source_rls_and_temporary_shadows_fail_closed \
  faults::postgres_session_tasks_search_path_changes_are_rejected_and_pool_scope_is_restored \
  concurrency::postgres_session_tasks_logout_waits_for_admitted_command_commit \
  concurrency::postgres_session_tasks_waiting_behind_logout_cannot_write \
  concurrency::postgres_session_tasks_expiry_is_checked_after_registry_and_event_waits \
  concurrency::postgres_session_tasks_cancelled_event_rolls_back_and_restores_source_pool \
  concurrency::postgres_session_tasks_membership_suspension_waits_then_denies_future_commands; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_tdd "$session_task_pg_case" -- --ignored --list | grep -Fx "$session_task_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_tdd "$session_task_pg_case" -- --ignored --exact --nocapture
done
