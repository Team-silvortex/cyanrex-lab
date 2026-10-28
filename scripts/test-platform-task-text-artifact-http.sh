#!/usr/bin/env bash
set -euo pipefail
# Standalone Task-bound HTTP only; never provisions storage, mounts hosts or retries requests.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
platform_task_text_http_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_task_text_http_root"
for platform_task_text_http_case in \
  behavior::postgres_platform_task_text_artifact_http_commits_private_bytes_without_task_mutation \
  ingress::postgres_platform_task_text_artifact_http_strict_dto_path_and_ingress_reject_before_io \
  authority::postgres_platform_task_text_artifact_http_current_owner_revision_and_duplicate_boundaries \
  faults::postgres_platform_task_text_artifact_http_deferred_task_drift_never_confirms \
  drift::postgres_platform_task_text_artifact_http_original_storage_grants_and_readonly_fail_closed \
  admission::postgres_platform_task_text_artifact_http_response_capacity_body_deadline_and_cancel; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_task_text_artifact_http_tdd "$platform_task_text_http_case" -- --ignored --list | grep -Fx "$platform_task_text_http_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_task_text_artifact_http_tdd "$platform_task_text_http_case" -- --ignored --exact
done
