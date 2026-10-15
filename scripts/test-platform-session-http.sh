#!/usr/bin/env bash
set -euo pipefail

# Fault injection belongs only to an explicitly supplied disposable database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
session_http_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$session_http_repo_root"

for session_http_pg_case in \
  behavior::postgres_session_http_reads_current_session_and_confirms_exact_logout_without_touching_other_sessions \
  behavior::postgres_session_http_expired_revoked_and_recreated_accounts_never_authenticate_old_cookies \
  behavior::postgres_session_http_loopback_logout_clears_only_the_host_platform_cookie_without_secure \
  faults::postgres_session_http_suppressed_or_redirected_logout_never_clears_a_still_valid_cookie \
  waits::postgres_session_http_get_rechecks_expiry_after_an_observed_source_row_lock_wait \
  waits::postgres_session_http_actual_delete_lock_timeout_returns_unconfirmed_without_cookie_clear \
  capacity_sql::postgres_session_http_held_response_and_cancelled_actual_logout_wait_preserve_independent_results; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_session_http_tdd "$session_http_pg_case" -- --ignored --list | grep -Fx "$session_http_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_session_http_tdd "$session_http_pg_case" -- --ignored --exact
done
