#!/usr/bin/env bash
set -euo pipefail

# Each case owns disposable storage and explicitly provisions its narrow runtime privileges.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
platform_replace_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_replace_repo_root"

for platform_replace_case in \
  behavior::postgres_platform_task_replace_empty_draft_title_commits_one_revision_event_without_file_changes \
  guards::postgres_platform_task_replace_revision_lifecycle_and_current_private_identity_never_widen_access \
  content::postgres_platform_task_replace_invalid_new_pins_and_corrupt_old_content_cannot_be_hidden \
  drift::postgres_platform_task_replace_original_root_and_equivalent_marker_replacements_are_not_adopted \
  faults::postgres_platform_task_replace_immediate_deferred_task_and_full_union_faults_never_commit \
  waits::postgres_platform_task_replace_observed_waits_expiry_cancellation_and_old_new_bytes_fail_closed \
  concurrency::postgres_platform_task_replace_concurrent_expected_revision_and_opposite_unions_have_no_partial_winners \
  permissions::postgres_platform_task_replace_requires_only_explicit_task_update_event_insert_and_original_read_profile \
  http::postgres_platform_task_replace_http_strict_manifest_private_outcomes_and_body_lifetime_capacity \
  host::postgres_platform_task_replace_real_loopback_host_edits_lists_and_reopens_exact_owned_content; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_task_replace_connection_tdd "$platform_replace_case" -- --ignored --list | grep -Fx "$platform_replace_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_task_replace_connection_tdd "$platform_replace_case" -- --ignored --exact
done
