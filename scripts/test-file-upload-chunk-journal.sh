#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable PostgreSQL + exclusive private fixtures, never live Engine composition.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
chunk_journal_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$chunk_journal_repo_root"

for chunk_journal_pg_case in \
  baseline::postgres_fresh_chunk_journal_install_is_acknowledged \
  cases::behavior::postgres_real_cross_chunk_confirmations_persist_reopen_and_never_allow_restage \
  cases::ordering::postgres_unknown_commit_acknowledgement_precedes_any_file_creation \
  cases::cancellation::postgres_cancelled_precommit_and_running_worker_preserve_exact_unknown_outcomes \
  cases::faults::postgres_unknown_insert_and_final_confirmation_faults_never_adopt_or_release \
  cases::binding::postgres_existing_unknown_coordinate_conflicts_across_source_and_root_drift \
  cases::finalization::postgres_postwrite_source_or_original_inode_drift_cannot_confirm \
  cases::finalization::postgres_postconfirmation_acknowledged_row_survives_late_caller_uncertainty \
  cases::reads::postgres_status_observation_never_accesses_source_or_files_or_restores_dispatch \
  cases::schema::postgres_journal_installation_schema_pins_and_bad_records_fail_closed \
  cases::capacity::postgres_chunk_capacity_and_concurrent_coordinate_admission_are_bounded; do
  cargo test --manifest-path engine/Cargo.toml --locked --test file_upload_chunk_journal_tdd \
    "$chunk_journal_pg_case" -- --ignored --list | grep -Fx "$chunk_journal_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test file_upload_chunk_journal_tdd \
    "$chunk_journal_pg_case" -- --ignored --exact --nocapture
done
