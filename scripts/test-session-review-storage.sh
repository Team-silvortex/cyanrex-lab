#!/usr/bin/env bash
set -euo pipefail

# Synthetic authorization faults must never target a deployment database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
review_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$review_repo_root"

for review_pg_case in \
  postgres_session_reviews_pin_history_targets_evidence_and_configured_human_policy \
  postgres_session_reviews_manager_grants_never_expand_private_review_or_artifact_access \
  postgres_session_reviews_validate_every_target_and_evidence_before_publishing \
  postgres_session_reviews_accept_bounded_ordered_targets_with_identical_cross_list_evidence \
  postgres_session_reviews_duplicate_ids_and_concurrent_amendments_have_one_winner \
  lifecycle::postgres_session_reviews_reject_rule_source_and_unconfigured_human_policy \
  lifecycle::postgres_session_reviews_unbound_missing_membership_and_revoked_sessions_cannot_act \
  lifecycle::postgres_session_reviews_retired_and_recreated_accounts_cannot_inherit_judgments \
  faults::postgres_session_reviews_post_write_evidence_and_session_faults_roll_back_mutations \
  faults::postgres_session_reviews_artifact_replacement_preserves_original_namespace_pin \
  faults::postgres_session_reviews_valid_review_clone_path_cannot_escape_namespace_checks \
  faults::postgres_session_reviews_suppressed_rows_and_deferred_commit_leave_inputs_unchanged \
  faults::postgres_session_reviews_corrupt_content_blocks_read_and_revise_without_repair \
  concurrency::postgres_session_reviews_reader_holds_review_lock_while_waiting_for_artifacts \
  concurrency::postgres_session_reviews_historical_read_waits_for_trusted_amendment \
  concurrency::postgres_session_reviews_expiry_after_resource_and_event_waits_returns_nothing \
  concurrency::postgres_session_reviews_logout_order_preserves_single_transaction_authority \
  concurrency::postgres_session_reviews_cancelled_write_rolls_back_without_touching_inputs; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_review_tdd "$review_pg_case" -- --ignored --list | grep -Fx "$review_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_review_tdd "$review_pg_case" -- --ignored --exact --nocapture
done
