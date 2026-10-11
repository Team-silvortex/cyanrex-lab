#!/usr/bin/env bash
set -euo pipefail

# Destructive fault fixtures require an explicitly disposable database, never ambient DATABASE_URL.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
index_http_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$index_http_repo_root"

for index_http_pg_case in \
  behavior::postgres_index_http_committed_mixed_metadata_pages_and_positional_cursors \
  privacy::postgres_index_http_teacher_isolation_and_untrusted_query_controls_are_read_only \
  lifecycle::postgres_index_http_lifecycle_and_account_recreation_reauthorize \
  faults::postgres_index_http_full_definitions_heads_manifests_and_lookahead_events_fail_closed \
  faults::postgres_index_http_empty_namespace_column_damage_is_not_empty_success \
  artifact_independence::postgres_index_http_metadata_does_not_read_artifact_namespace_or_blobs \
  waits::postgres_index_http_real_row_wait_expiry_discards_metadata \
  capacity_sql::postgres_index_http_real_wait_cancellation_and_response_lifetime_release_capacity; do
  cargo test --manifest-path engine/Cargo.toml --locked --test task_content_index_http_tdd "$index_http_pg_case" -- --ignored --list | grep -Fx "$index_http_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test task_content_index_http_tdd "$index_http_pg_case" -- --ignored --exact --nocapture
done
