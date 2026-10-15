#!/usr/bin/env bash
set -euo pipefail

# Installs synthetic templates only in its own database, never through the inspected API.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
storage_markers_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$storage_markers_repo_root"

for storage_markers_case in \
  behavior::postgres_platform_storage_markers_observe_real_templates_with_only_marker_column_permissions \
  rejections::postgres_platform_storage_markers_reject_missing_versions_scopes_and_catalog_dependencies \
  shapes::postgres_platform_storage_markers_reject_unsafe_relations_and_metadata_column_shapes \
  pins::postgres_platform_storage_markers_refuse_waited_replacement_type_and_row_security_drift; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_storage_markers_tdd "$storage_markers_case" -- --ignored --list | grep -Fx "$storage_markers_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_storage_markers_tdd "$storage_markers_case" -- --ignored --exact
done
