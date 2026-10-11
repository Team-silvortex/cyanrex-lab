#!/usr/bin/env bash
set -euo pipefail

# Observation fault/wait fixtures require an explicitly selected disposable database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
catalog_observation_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$catalog_observation_repo_root"

for catalog_observation_pg_case in \
  behavior::postgres_catalog_observation_preflight_ready_complete_and_replaced_session_never_reads \
  behavior::postgres_catalog_observation_artifact_match_differs_historical_revision_and_prefix_scope \
  behavior::postgres_catalog_observation_task_full_definitions_versions_empty_order_and_edits_never_adopt \
  faults::postgres_catalog_observation_missing_foreign_and_missing_task_scope_remain_not_visible \
  faults::postgres_catalog_observation_effective_commit_failures_and_invalid_definition_bytes_remain_typed \
  lifecycle::postgres_catalog_observation_original_session_reauthenticates_each_read_and_lock_expiry_discards_results \
  concurrency::postgres_catalog_observation_cancelled_artifact_and_task_reads_preserve_all_state_rows_files_and_pool \
  concurrency::postgres_catalog_observation_pending_creator_visibility_cannot_prove_previous_or_later_commit; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_catalog_draft_observation_tdd "$catalog_observation_pg_case" -- --ignored --list | grep -Fx "$catalog_observation_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_catalog_draft_observation_tdd "$catalog_observation_pg_case" -- --ignored --exact --nocapture
done
