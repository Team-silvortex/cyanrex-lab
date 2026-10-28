#!/usr/bin/env bash
set -euo pipefail
# Explicit original-storage opt-in host evidence; no provisioning, trust changes or retries.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
platform_task_publication_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_task_publication_root"
for platform_task_publication_case in \
  workspace::postgres_platform_task_text_publication_workspace_nine_routes_and_opt_in_boundaries \
  boundaries::postgres_platform_task_text_publication_original_binding_owner_revision_and_grants \
  capacity::postgres_platform_task_text_publication_eight_response_lanes_and_capacity_validation \
  native::postgres_platform_task_text_publication_plain_flag_login_publish_and_shutdown \
  native::postgres_platform_task_text_publication_tls_flag_private_ca_publish_and_shutdown \
  faults::postgres_platform_task_text_publication_deferred_task_drift_preserves_unknown_blob; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_task_text_publication_host_tdd "$platform_task_publication_case" -- --ignored --list | grep -Fx "$platform_task_publication_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_task_text_publication_host_tdd "$platform_task_publication_case" -- --ignored --exact
done
