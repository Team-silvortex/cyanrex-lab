#!/usr/bin/env bash
set -euo pipefail

# Fault injection is restricted to the caller's explicitly disposable database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
draft_journal_observation_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$draft_journal_observation_repo_root"

for draft_journal_observation_case in \
  behavior::postgres_draft_journal_observation_matches_committed_artifacts_and_tasks \
  behavior::postgres_draft_journal_observation_unknown_absent_or_present_never_resumes \
  behavior::postgres_draft_journal_observation_filters_missing_foreign_and_unrecorded_targets \
  behavior::postgres_draft_journal_observation_reports_later_task_edits_as_different \
  faults::postgres_draft_journal_observation_rejects_schema_installation_and_routing_damage \
  faults::postgres_draft_journal_observation_rejects_corrupt_intents_and_step_sequences \
  faults::postgres_draft_journal_observation_reads_exact_old_revision_and_distinguishes_metadata \
  faults::postgres_draft_journal_observation_preserves_typed_blob_and_permission_errors \
  faults::postgres_draft_journal_observation_missing_task_does_not_probe_artifact_namespace \
  lifecycle::postgres_draft_journal_observation_requires_current_session_and_membership \
  lifecycle::postgres_draft_journal_observation_resource_wait_expiry_and_cancellation_are_read_only \
  concurrency::postgres_draft_journal_observation_holds_original_journal_guard_across_resource_wait; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_draft_journal_observation_tdd "$draft_journal_observation_case" -- --ignored --list | grep -Fx "$draft_journal_observation_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_draft_journal_observation_tdd "$draft_journal_observation_case" -- --ignored --exact --nocapture
done
