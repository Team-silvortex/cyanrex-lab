#!/usr/bin/env bash
set -euo pipefail

# Every case owns its database/role; no ambient Engine URL or privilege repair.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
platform_content_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_content_repo_root"

for platform_content_case in \
  behavior::postgres_platform_task_content_reopens_empty_and_ordered_exact_unicode_revisions \
  behavior::postgres_platform_task_content_current_identity_and_private_owner_never_inherit_access \
  drift::postgres_platform_task_content_original_root_and_equivalent_marker_replacements_are_not_adopted \
  content::postgres_platform_task_content_corrupt_metadata_or_exact_bytes_never_returns_partial_content \
  waits::postgres_platform_task_content_observed_task_and_artifact_waits_recheck_final_expiry \
  permissions::postgres_platform_task_content_narrow_runtime_privileges_never_fall_back_or_auto_grant \
  http::postgres_platform_task_content_http_read_only_wire_observed_cancel_and_body_capacity \
  host::postgres_platform_task_content_real_loopback_host_reopens_private_work_without_write_routes; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_task_content_connection_tdd "$platform_content_case" -- --ignored --list | grep -Fx "$platform_content_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_task_content_connection_tdd "$platform_content_case" -- --ignored --exact
done
