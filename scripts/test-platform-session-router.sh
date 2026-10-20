#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable administration only; runtime roles use synthetic fixture passwords.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL URL with runtime-role password authentication}"
platform_session_router_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_session_router_repo_root"

for platform_session_router_case in \
  behavior::postgres_platform_session_router_confirmed_login_query_logout_share_original_binding \
  admission::postgres_platform_session_router_clones_share_login_rate_without_blocking_lifecycle \
  admission::postgres_platform_session_router_response_capacity_is_shared_per_lane_and_released_independently \
  faults::postgres_platform_session_router_original_root_and_marker_drift_reject_both_lanes_without_cached_auth \
  faults::postgres_platform_session_router_readonly_and_missing_delete_never_clear_unconfirmed_logout \
  lifetime::postgres_platform_session_router_cancelled_observed_sql_wait_drains_without_manufacturing_success \
  lifetime::postgres_platform_session_router_cancelled_confirmed_delivery_preserves_committed_login_or_logout; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_session_router_tdd "$platform_session_router_case" -- --ignored --list | grep -Fx "$platform_session_router_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_session_router_tdd "$platform_session_router_case" -- --ignored --exact
done
