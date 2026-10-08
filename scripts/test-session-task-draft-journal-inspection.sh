#!/usr/bin/env bash
set -euo pipefail

# These fault-injection cases require an explicitly disposable database, never a deployment URL.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
draft_journal_inspection_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$draft_journal_inspection_repo_root"

for draft_journal_inspection_case in \
  behavior::postgres_draft_journal_inspection_reopens_fresh_session_and_reports_recorded_prefixes \
  behavior::postgres_draft_journal_inspection_unknown_steps_do_not_resume_publication \
  behavior::postgres_draft_journal_inspection_missing_foreign_and_account_mismatch_skip_steps \
  faults::postgres_draft_journal_inspection_requires_schema_two_and_exact_installation \
  faults::postgres_draft_journal_inspection_rejects_changed_namespace_routes \
  faults::postgres_draft_journal_inspection_rejects_noncanonical_and_oversized_intents \
  faults::postgres_draft_journal_inspection_rejects_invalid_step_sequences_and_bounds \
  lifecycle::postgres_draft_journal_inspection_requires_current_session_and_membership \
  lifecycle::postgres_draft_journal_inspection_rechecks_expiry_after_journal_wait \
  concurrency::postgres_draft_journal_inspection_cancelled_and_concurrent_reads_leave_resources_untouched; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_draft_journal_inspection_tdd "$draft_journal_inspection_case" -- --ignored --list | grep -Fx "$draft_journal_inspection_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_task_draft_journal_inspection_tdd "$draft_journal_inspection_case" -- --ignored --exact --nocapture
done
