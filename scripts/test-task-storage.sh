#!/usr/bin/env bash
set -euo pipefail

# Use a disposable PostgreSQL database, never a deployment database. Each case owns its schema.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
task_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$task_repo_root"

for task_work_pg_case in \
  postgres_tasks_keep_manual_and_catalog_work_across_reopen \
  postgres_tasks_scope_owner_and_input_boundaries_are_explicit \
  postgres_tasks_transitions_pin_content_and_reject_stale_or_terminal_writes \
  postgres_tasks_concurrent_creation_and_revision_change_have_one_winner \
  faults::postgres_tasks_install_only_explicit_empty_namespaces \
  faults::postgres_tasks_failed_or_suppressed_events_roll_back_the_task \
  faults::postgres_tasks_state_or_event_tampering_is_rejected_before_publication \
  faults::postgres_tasks_deferred_commit_failure_returns_no_success_or_partial_event \
  faults::postgres_tasks_cancelled_write_before_event_commit_is_rolled_back \
  faults::postgres_tasks_missing_event_and_rls_tables_fail_closed \
  faults::postgres_tasks_temporary_shadow_tables_are_not_authoritative \
  faults::postgres_tasks_post_write_scope_changes_roll_back_before_success \
  faults::postgres_tasks_readers_observe_one_committed_snapshot_during_a_write \
  faults::postgres_tasks_writers_wait_for_metadata_before_admission \
  faults::postgres_tasks_revision_overflow_does_not_wrap_or_append_an_event; do
  cargo test --manifest-path engine/Cargo.toml --locked --test task_store_tdd "$task_work_pg_case" -- --ignored --list | grep -Fx "$task_work_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test task_store_tdd "$task_work_pg_case" -- --ignored --exact --nocapture
done
