#!/usr/bin/env bash
set -euo pipefail
# Unmounted Task-bound publication only; never installs storage, grants, retries or host routes.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
platform_task_text_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_task_text_root"
for platform_task_text_case in \
  behavior::postgres_platform_task_text_artifact_commits_private_bytes_without_mutating_task_or_source \
  guards::postgres_platform_task_text_artifact_cross_owner_missing_and_scope_never_create_files \
  guards::postgres_platform_task_text_artifact_stale_tampered_status_and_revoked_reject_before_publication \
  faults::postgres_platform_task_text_artifact_deferred_task_drift_rolls_back_rows_but_preserves_uncertain_blob \
  drift::postgres_platform_task_text_artifact_original_root_and_marker_identity_are_not_recaptured \
  permissions::postgres_platform_task_text_artifact_narrow_insert_task_lock_and_readonly_profile_are_not_repaired \
  waits::postgres_platform_task_text_artifact_observed_task_wait_rechecks_session_before_new_file; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_task_text_artifact_connection_tdd "$platform_task_text_case" -- --ignored --list | grep -Fx "$platform_task_text_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_task_text_artifact_connection_tdd "$platform_task_text_case" -- --ignored --exact
done
