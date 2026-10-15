#!/usr/bin/env bash
set -euo pipefail

# Creates and removes its own database and runtime role inside an explicit disposable cluster.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
host_preflight_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$host_preflight_repo_root"

for host_preflight_case in \
  behavior::postgres_host_preflight_catalog_observation_is_read_only_on_empty_unprivileged_namespaces \
  boundaries::postgres_host_preflight_rejects_wrong_targets_outer_transactions_and_missing_catalog_permission \
  roles::postgres_host_preflight_rejects_unsafe_runtime_flags_memberships_and_database_privileges \
  namespaces::postgres_host_preflight_rejects_unsafe_namespaces_and_invalidates_original_catalog_observations; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_host_preflight_tdd "$host_preflight_case" -- --ignored --list | grep -Fx "$host_preflight_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_host_preflight_tdd "$host_preflight_case" -- --ignored --exact
done
