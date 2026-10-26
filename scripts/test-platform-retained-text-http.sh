#!/usr/bin/env bash
set -euo pipefail

# Six exact retained HTTP boundaries. Initialization and target selection are caller-owned.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
retained_text_http_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$retained_text_http_root"

for retained_text_http_case in \
  behavior::postgres_platform_text_artifact_http_private_roundtrip_and_separate_task_reference_bridge \
  admission::postgres_platform_text_artifact_http_bad_admission_and_strict_body_never_write \
  authority::postgres_platform_text_artifact_http_current_authority_and_duplicate_ids_are_private \
  faults::postgres_platform_text_artifact_http_deferred_faults_are_unconfirmed_and_orphans_never_adopt \
  boundaries::postgres_platform_text_artifact_http_original_binding_and_minimal_privileges_are_not_repaired \
  flow::postgres_platform_text_artifact_http_observed_sql_wait_expiry_cancel_and_response_capacity; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_text_artifact_http_tdd "$retained_text_http_case" -- --ignored --list | grep -Fx "$retained_text_http_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_text_artifact_http_tdd "$retained_text_http_case" -- --ignored --exact
done
