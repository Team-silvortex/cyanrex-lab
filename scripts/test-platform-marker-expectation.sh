#!/usr/bin/env bash
set -euo pipefail

# Fresh fixture runtime roles must authenticate using their explicitly configured password.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL URL with runtime-role password authentication}"
platform_marker_expectation_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_marker_expectation_repo_root"

for platform_marker_expectation_case in \
  behavior::postgres_platform_marker_expectation_observes_real_original_markers_without_adoption_or_mutation \
  boundaries::postgres_platform_marker_expectation_checks_all_identity_slots_and_five_first_call_replacements \
  boundaries::postgres_platform_marker_expectation_rejects_owner_drift_and_preserves_outer_transactions \
  diagnostic::postgres_platform_marker_expectation_diagnostic_uses_one_fresh_explicit_attempt_and_closes \
  cli::postgres_platform_marker_expectation_cli_v3_rejects_original_marker_owner_while_v2_is_diagnostic; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_marker_expectation_tdd "$platform_marker_expectation_case" -- --ignored --list | grep -Fx "$platform_marker_expectation_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_marker_expectation_tdd "$platform_marker_expectation_case" -- --ignored --exact
done
