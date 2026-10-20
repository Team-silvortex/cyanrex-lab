#!/usr/bin/env bash
set -euo pipefail

# Explicit owned database/role fault fixtures only, never an ambient Engine DATABASE_URL.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
platform_index_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_index_repo_root"

for platform_index_case in \
  behavior::postgres_platform_task_index_private_empty_payload_pages_derive_scope_and_owner \
  behavior::postgres_platform_task_index_invalid_unbound_and_suspended_sessions_never_receive_metadata \
  drift::postgres_platform_task_index_original_root_and_marker_replacements_are_not_adopted \
  waits::postgres_platform_task_index_observed_task_wait_rechecks_expiry_and_original_namespace_owner \
  content::postgres_platform_task_index_checks_full_lookahead_without_reading_artifact_rows_or_bytes \
  permissions::postgres_platform_task_index_narrow_runtime_acl_and_missing_privilege_never_fall_back \
  http::postgres_platform_task_index_http_observed_wait_cancellation_private_wire_and_body_capacity \
  host::postgres_platform_task_index_real_loopback_host_lists_private_zero_payload_work_and_drains; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_task_index_connection_tdd "$platform_index_case" -- --ignored --list | grep -Fx "$platform_index_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_task_index_connection_tdd "$platform_index_case" -- --ignored --exact
done
