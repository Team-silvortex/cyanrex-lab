#!/usr/bin/env bash
set -euo pipefail

# Every case owns its database and role. Never select an ambient Engine URL or repair privileges.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
platform_create_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_create_repo_root"

for platform_create_case in \
  behavior::postgres_platform_task_create_commits_empty_draft_and_owner_private_list_reopen \
  behavior::postgres_platform_task_create_duplicates_and_current_identity_never_adopt_an_owner \
  drift::postgres_platform_task_create_original_root_and_equivalent_markers_are_never_adopted \
  faults::postgres_platform_task_create_immediate_and_deferred_side_effects_never_confirm_a_task \
  waits::postgres_platform_task_create_observed_event_wait_expiry_and_cancellation_never_confirm \
  permissions::postgres_platform_task_create_requires_narrow_insert_privileges_and_explicit_read_write_role \
  http::postgres_platform_task_create_http_strict_wire_observed_cancel_and_response_capacity \
  host::postgres_platform_task_create_real_loopback_host_creates_lists_and_reopens_private_empty_work; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_task_create_connection_tdd "$platform_create_case" -- --ignored --list | grep -Fx "$platform_create_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_task_create_connection_tdd "$platform_create_case" -- --ignored --exact
done
