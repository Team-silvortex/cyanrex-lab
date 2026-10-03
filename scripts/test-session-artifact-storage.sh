#!/usr/bin/env bash
set -euo pipefail

# Authorization and publication fault fixtures require an explicitly disposable database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
session_artifact_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$session_artifact_repo_root"

for session_artifact_pg_case in \
  postgres_session_artifacts_derive_owner_and_preserve_exact_old_content \
  postgres_session_artifacts_managers_cannot_touch_private_content_even_when_blob_is_missing \
  postgres_session_artifacts_revision_races_and_duplicate_ids_do_not_write_extra_files \
  postgres_session_artifacts_exact_references_never_grant_cross_scope_or_digest_access \
  lifecycle::postgres_session_artifacts_unbound_missing_membership_and_revoked_sessions_never_write \
  lifecycle::postgres_session_artifacts_retired_and_recreated_accounts_cannot_inherit_content \
  faults::postgres_session_artifacts_invalid_sources_scopes_and_namespaces_write_no_files \
  faults::postgres_session_artifacts_event_and_commit_failures_retain_unpublished_files \
  faults::postgres_session_artifacts_post_write_authority_changes_roll_back_but_keep_files \
  faults::postgres_session_artifacts_source_rls_and_temporary_shadows_write_no_files \
  faults::postgres_session_artifacts_valid_clone_namespace_switch_cannot_publish \
  faults::postgres_session_artifacts_replaced_roots_and_corrupt_bytes_never_escape \
  concurrency::postgres_session_artifacts_logout_waits_for_admitted_publication_commit \
  concurrency::postgres_session_artifacts_waiting_behind_logout_never_write_blobs \
  concurrency::postgres_session_artifacts_expiry_after_registry_and_event_waits_never_publishes \
  concurrency::postgres_session_artifacts_cancelled_revision_retains_file_and_restores_source_pool \
  concurrency::postgres_session_artifacts_membership_suspension_waits_then_denies_read_and_revision \
  concurrency::postgres_session_artifacts_exact_read_waits_for_trusted_revision_without_mixed_head; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_artifact_tdd "$session_artifact_pg_case" -- --ignored --list | grep -Fx "$session_artifact_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_artifact_tdd "$session_artifact_pg_case" -- --ignored --exact --nocapture
done
