#!/usr/bin/env bash
set -euo pipefail

# Runtime roles need password authentication; admin-only local trust is permitted.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL URL with runtime-role password authentication}"
platform_catalog_expectation_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_catalog_expectation_repo_root"

for platform_catalog_expectation_case in \
  behavior::postgres_platform_catalog_expectation_connects_fresh_and_checks_all_config_and_identity_fields \
  behavior::postgres_platform_catalog_expectation_rejects_same_name_namespace_replacement_before_first_connect \
  drift::postgres_platform_catalog_expectation_rechecks_role_namespace_owner_and_privilege_drift \
  cli::postgres_platform_catalog_expectation_cli_v2_rejects_original_owner_drift_while_v1_remains_diagnostic; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_catalog_expectation_tdd "$platform_catalog_expectation_case" -- --ignored --list | grep -Fx "$platform_catalog_expectation_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_catalog_expectation_tdd "$platform_catalog_expectation_case" -- --ignored --exact
done
