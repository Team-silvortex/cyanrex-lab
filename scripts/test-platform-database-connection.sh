#!/usr/bin/env bash
set -euo pipefail

: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
platform_connection_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_connection_repo_root"

for platform_connection_case in \
  behavior::postgres_platform_connection_opens_fresh_verified_read_only_connections_without_mutation \
  identity::postgres_platform_connection_rejects_same_name_namespace_replacement_without_adoption \
  identity::postgres_platform_connection_rejects_namespace_owner_drift_until_exact_restoration \
  privileges::postgres_platform_connection_rechecks_unsafe_privileges_and_control_permission_each_time; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_database_connection_tdd "$platform_connection_case" -- --ignored --list | grep -Fx "$platform_connection_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_database_connection_tdd "$platform_connection_case" -- --ignored --exact
done
