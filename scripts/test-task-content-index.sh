#!/usr/bin/env bash
set -euo pipefail

# Fault and lifecycle fixtures must use only an explicitly disposable PostgreSQL database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
content_index_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$content_index_repo_root"

run_content_index_case() {
  cargo test --manifest-path engine/Cargo.toml --locked --test "$1" "$2" -- --ignored --list | grep -Fx "$2: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test "$1" "$2" -- --ignored --exact --nocapture
}

for content_index_pg_case in \
  index_trusted::postgres_content_index_orders_bounds_and_projects_mixed_tasks \
  index_trusted::postgres_content_index_rejects_corrupt_selected_and_lookahead_rows \
  index_trusted::postgres_content_index_owner_cursor_scope_and_schema_are_fail_closed \
  index_trusted::postgres_content_index_cancelled_table_wait_releases_resources_without_writes; do
  run_content_index_case task_content_index_tdd "$content_index_pg_case"
done

for content_index_pg_case in \
  behavior::postgres_session_content_index_empty_default_and_bounded_uuid_pages_are_read_only \
  behavior::postgres_session_content_index_mixed_definitions_owner_cursor_and_namespace_isolation \
  validation::postgres_session_content_index_full_definition_and_lookahead_fail_closed_without_filtering \
  validation::postgres_session_content_index_manifest_head_event_and_schema_damage_never_yield_partial_pages \
  artifact_independence::postgres_session_content_index_never_reads_artifact_bytes_or_artifact_namespace \
  lifecycle::postgres_session_content_index_current_sessions_memberships_and_recreated_accounts_reauthorize \
  waits::postgres_session_content_index_real_task_row_wait_then_expiry_discards_the_page \
  waits::postgres_session_content_index_cancelled_real_row_wait_preserves_rows_files_and_pool; do
  run_content_index_case session_task_content_index_tdd "$content_index_pg_case"
done
