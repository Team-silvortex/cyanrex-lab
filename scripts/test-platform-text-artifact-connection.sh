#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable administration only; this runner never chooses or initializes storage.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
platform_text_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_text_repo_root"

for platform_text_case in \
  behavior::postgres_platform_text_artifact_commits_exact_empty_unicode_and_maximum_text \
  behavior::postgres_platform_text_artifact_invalid_text_title_and_token_never_dispatch \
  lifecycle::postgres_platform_text_artifact_current_private_owner_and_account_incarnation_remain_authority \
  duplicates::postgres_platform_text_artifact_duplicate_ids_concurrent_publication_and_orphans_never_adopt \
  drift::postgres_platform_text_artifact_original_root_and_equal_marker_clones_are_never_adopted \
  faults::postgres_platform_text_artifact_immediate_and_deferred_publication_faults_never_confirm \
  waits::postgres_platform_text_artifact_observed_deferred_waits_recheck_expiry_bytes_root_and_cancel \
  permissions::postgres_platform_text_artifact_narrow_insert_profile_and_read_only_policy_are_not_repaired; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_text_artifact_connection_tdd "$platform_text_case" -- --ignored --list | grep -Fx "$platform_text_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_text_artifact_connection_tdd "$platform_text_case" -- --ignored --exact
done
