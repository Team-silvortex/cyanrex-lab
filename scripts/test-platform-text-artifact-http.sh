#!/usr/bin/env bash
set -euo pipefail

# Explicit, unmounted text transport: only an explicitly selected disposable database is allowed.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
text_artifact_http_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$text_artifact_http_repo_root"

for text_artifact_http_case in \
  behavior::postgres_text_artifact_http_roundtrip_and_task_manifest_bridge_preserve_exact_text \
  behavior::postgres_text_artifact_http_reads_exact_history_and_rejects_wrong_digest \
  behavior::postgres_text_artifact_http_preflight_and_private_owner_do_not_probe_or_write \
  behavior::postgres_text_artifact_http_valid_generic_artifacts_outside_text_profile_remain_private \
  faults::postgres_text_artifact_http_current_session_and_membership_gate_every_read_and_create \
  faults::postgres_text_artifact_http_expiry_after_exact_storage_wait_never_confirms_or_discloses \
  faults::postgres_text_artifact_http_outbox_and_commit_faults_preserve_orphans_without_adoption \
  faults::postgres_text_artifact_http_schema_root_and_blob_failures_never_return_text_or_repair \
  concurrency::postgres_text_artifact_http_duplicate_concurrent_ids_conflict_without_adoption \
  concurrency::postgres_text_artifact_http_capacity_covers_sql_wait_and_response_without_cancel_cleanup; do
  cargo test --manifest-path engine/Cargo.toml --locked --test text_artifact_http_tdd "$text_artifact_http_case" -- --ignored --list | grep -Fx "$text_artifact_http_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test text_artifact_http_tdd "$text_artifact_http_case" -- --ignored --exact --nocapture
done
