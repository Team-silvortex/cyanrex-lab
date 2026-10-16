#!/usr/bin/env bash
set -euo pipefail

# Use only an explicitly disposable administrator fixture; runtime roles authenticate by password.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL URL with runtime-role password authentication}"
platform_session_connection_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_session_connection_repo_root"

for platform_session_connection_case in \
  behavior::postgres_platform_session_connection_minimal_role_queries_and_logs_out_existing_sessions \
  failures::postgres_platform_session_connection_rejects_readonly_and_missing_runtime_permissions_without_cookie_clear \
  failures::postgres_platform_session_connection_preserves_original_marker_and_root_bindings \
  waits::postgres_platform_session_connection_uses_fresh_expiry_after_real_source_fence_wait \
  triggers::postgres_platform_session_connection_delete_path_redirection_rolls_back_without_clearing_cookie \
  waits::postgres_platform_session_connection_rechecks_original_owner_after_source_wait_and_cancellation \
  triggers::postgres_platform_session_connection_flushes_deferred_delete_reinsertion_before_success; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_session_connection_tdd "$platform_session_connection_case" -- --ignored --list | grep -Fx "$platform_session_connection_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_session_connection_tdd "$platform_session_connection_case" -- --ignored --exact
done
