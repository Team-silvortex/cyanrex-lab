#!/usr/bin/env bash
set -euo pipefail

# Disposable PostgreSQL only. Files are created under private test-owned temporary directories.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
artifact_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$artifact_repo_root"

for artifact_pg_case in \
  postgres_artifacts_preserve_exact_bytes_and_immutable_revisions_after_reopen \
  postgres_artifacts_owner_scope_and_digest_are_not_interchangeable \
  postgres_artifacts_concurrent_writes_and_stale_parents_have_one_winner \
  postgres_artifacts_task_inputs_keep_original_content_after_new_revision \
  faults::postgres_artifacts_install_only_empty_namespaces_and_pin_scope \
  faults::postgres_artifacts_event_failure_rolls_back_metadata_but_retains_unpublished_files \
  faults::postgres_artifacts_deferred_commit_failure_never_publishes_metadata \
  faults::postgres_artifacts_cancelled_publication_keeps_evidence_without_partial_metadata \
  faults::postgres_artifacts_reject_missing_changed_and_oversized_content \
  faults::postgres_artifacts_reject_symbolic_hard_links_and_special_files \
  faults::postgres_artifacts_replaced_blob_root_cannot_acknowledge_publication \
  faults::postgres_artifacts_readers_see_committed_revisions_during_publication \
  faults::postgres_artifacts_post_write_scope_and_event_tampering_roll_back \
  faults::postgres_artifacts_reject_rls_missing_events_and_temporary_shadow_tables \
  faults::postgres_artifacts_existing_files_and_revision_ids_are_never_overwritten_or_adopted \
  faults::postgres_artifacts_rewound_heads_cannot_hide_newer_published_revisions \
  faults::postgres_artifacts_bound_persisted_payloads_and_reject_unsupported_contracts; do
  cargo test --manifest-path engine/Cargo.toml --locked --test artifact_store_tdd "$artifact_pg_case" -- --ignored --list | grep -Fx "$artifact_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test artifact_store_tdd "$artifact_pg_case" -- --ignored --exact --nocapture
done
