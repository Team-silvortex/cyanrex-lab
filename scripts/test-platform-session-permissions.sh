#!/usr/bin/env bash
set -euo pipefail

: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
session_acl_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$session_acl_repo_root"

for session_acl_case in \
  behavior::postgres_session_permissions_column_only_role_reads_and_logs_out_without_credentials \
  rejections::postgres_session_permissions_missing_select_update_or_delete_never_mutates_or_clears_cookie \
  waits::postgres_session_permissions_source_guard_holds_three_relation_locks_and_blocks_real_ddl \
  shapes::postgres_session_permissions_parse_only_rejects_view_rls_and_preserves_a_colliding_plan; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_session_permissions_tdd "$session_acl_case" -- --ignored --list | grep -Fx "$session_acl_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_session_permissions_tdd "$session_acl_case" -- --ignored --exact
done
