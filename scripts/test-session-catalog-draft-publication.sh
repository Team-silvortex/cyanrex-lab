#!/usr/bin/env bash
set -euo pipefail

# Publication fault/wait fixtures require an explicitly selected disposable database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
catalog_draft_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$catalog_draft_repo_root"

for catalog_draft_pg_case in \
  behavior::postgres_catalog_publication_packages_versions_bindings_and_ordered_text_confirm_one_write_at_a_time \
  behavior::postgres_catalog_publication_unknown_coordinates_and_session_substitution_never_dispatch \
  faults::postgres_catalog_publication_second_artifact_fault_preserves_prefix_orphan_and_terminal_target \
  faults::postgres_catalog_publication_final_task_definition_outbox_session_and_commit_faults_never_confirm_or_retry \
  faults::postgres_catalog_publication_damaged_confirmed_bytes_and_preallocated_conflicts_are_not_adopted \
  lifecycle::postgres_catalog_publication_each_artifact_and_task_step_reauthenticates_current_session_and_membership \
  waits::postgres_catalog_publication_observed_artifact_and_task_waits_keep_cancellation_unconfirmed \
  waits::postgres_catalog_publication_observed_artifact_and_task_waits_recheck_database_session_expiry; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_catalog_draft_publication_tdd "$catalog_draft_pg_case" -- --ignored --list | grep -Fx "$catalog_draft_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_catalog_draft_publication_tdd "$catalog_draft_pg_case" -- --ignored --exact --nocapture
done
