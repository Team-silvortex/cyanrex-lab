#!/usr/bin/env bash
set -euo pipefail

# Explicit catalogue HTTP preparation uses only disposable, opt-in PostgreSQL fixtures.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
catalog_http_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$catalog_http_repo_root"

for catalog_http_pg_case in \
  behavior::postgres_catalog_http_mixed_frozen_packages_versions_and_bindings_roundtrip_all_four_commands \
  privacy::postgres_catalog_http_manual_catalog_and_owner_privacy_are_bidirectionally_disjoint \
  metadata::postgres_catalog_http_full_metadata_binding_drift_and_unknown_exact_coordinates_never_write \
  validation::postgres_catalog_http_invalid_new_text_and_corrupt_saved_bytes_cannot_be_hidden \
  lifecycle::postgres_catalog_http_current_sessions_membership_and_recreated_owner_never_inherit_content \
  faults::postgres_catalog_http_post_write_definition_bytes_session_and_commit_faults_stay_unconfirmed \
  waits::postgres_catalog_http_expiry_at_observed_read_write_wait_and_post_write_blob_loss_fail_closed \
  capacity_sql::postgres_catalog_http_shared_budget_covers_committed_body_sql_wait_cancel_and_eof; do
  cargo test --manifest-path engine/Cargo.toml --locked --test catalog_task_content_http_tdd "$catalog_http_pg_case" -- --ignored --list | grep -Fx "$catalog_http_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test catalog_task_content_http_tdd "$catalog_http_pg_case" -- --ignored --exact --nocapture
done
