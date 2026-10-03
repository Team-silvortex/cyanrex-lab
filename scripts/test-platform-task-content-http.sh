#!/usr/bin/env bash
set -euo pipefail

# Prepared HTTP routes exercise durable Session transactions only on disposable storage.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
platform_http_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_http_repo_root"

for platform_http_pg_case in \
  behavior::postgres_platform_http_create_read_preserves_exact_unicode_order_and_empty_payload \
  behavior::postgres_platform_http_replace_status_and_conflicts_return_exact_server_revisions \
  behavior::postgres_platform_http_private_ownership_and_forged_pins_never_expose_content \
  behavior::postgres_platform_http_preflight_rejections_cannot_change_a_valid_session_task \
  behavior::postgres_platform_http_new_invalid_text_and_corrupt_old_bytes_cannot_be_saved_or_erased \
  faults::postgres_platform_http_session_logout_expiry_and_membership_are_not_cached_authority \
  faults::postgres_platform_http_expiry_after_admission_and_storage_wait_returns_no_content_or_commit \
  faults::postgres_platform_http_commit_and_outbox_faults_remain_unconfirmed_without_cookie_clearing \
  faults::postgres_platform_http_post_write_old_blob_loss_is_rechecked_before_http_success \
  faults::postgres_platform_http_schema_two_is_not_upgraded_or_used_as_a_fallback \
  faults::postgres_platform_http_competing_edits_have_one_success_and_one_stale_response; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_http_tdd "$platform_http_pg_case" -- --ignored --list | grep -Fx "$platform_http_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_http_tdd "$platform_http_pg_case" -- --ignored --exact --nocapture
done
