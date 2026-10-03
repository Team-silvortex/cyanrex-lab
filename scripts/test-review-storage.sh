#!/usr/bin/env bash
set -euo pipefail

# Fault injection and cross-store fixtures belong only in an explicitly disposable database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
review_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$review_repo_root"

for review_pg_case in \
  postgres_reviews_preserve_pins_and_comment_history_after_reopen \
  postgres_reviews_reviewer_scope_and_exact_revision_are_not_interchangeable \
  postgres_reviews_concurrent_amendments_have_one_winner \
  postgres_reviews_rule_observations_cannot_be_amended_to_human_judgments \
  integration::postgres_reviews_artifact_edits_keep_old_judgments_and_task_state \
  faults::postgres_reviews_install_only_empty_namespaces_and_pin_scope \
  faults::postgres_reviews_suppressed_events_roll_back_creation_and_amendment \
  faults::postgres_reviews_deferred_commit_failure_returns_no_success \
  faults::postgres_reviews_cancelled_write_before_commit_has_no_partial_judgment \
  faults::postgres_reviews_readers_see_committed_history_during_amendment \
  faults::postgres_reviews_post_write_scope_or_event_tampering_rolls_back \
  faults::postgres_reviews_missing_events_rls_and_temporary_shadows_fail_closed \
  faults::postgres_reviews_rewound_heads_cannot_hide_later_judgments \
  faults::postgres_reviews_payload_bounds_and_changed_contracts_fail_closed \
  faults::postgres_reviews_metadata_lock_timeout_cannot_publish \
  faults::postgres_reviews_revision_overflow_does_not_publish_an_amendment \
  faults::postgres_reviews_historical_reads_validate_events_and_unchanged_subject; do
  cargo test --manifest-path engine/Cargo.toml --locked --test review_store_tdd "$review_pg_case" -- --ignored --list | grep -Fx "$review_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test review_store_tdd "$review_pg_case" -- --ignored --exact --nocapture
done
