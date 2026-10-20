#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable administration only; each runtime role authenticates using its fixture password.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL URL with runtime-role password authentication}"
platform_login_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$platform_login_repo_root"

for platform_login_case in \
  behavior::postgres_platform_login_connection_exact_issuer_role_and_existing_session_lifecycle \
  behavior::postgres_platform_login_connection_wrong_missing_and_rate_limited_attempts_do_not_write \
  behavior::postgres_platform_login_connection_two_connectors_accept_one_otp_once \
  behavior::postgres_platform_login_connection_full_stock_preserves_otp_for_explicit_retry \
  faults::postgres_platform_login_connection_missing_credential_columns_preserve_every_row \
  faults::postgres_platform_login_connection_missing_issuer_writes_roll_back_otp_and_session \
  faults::postgres_platform_login_connection_explicit_readonly_policy_is_not_overridden \
  faults::postgres_platform_login_connection_retains_original_root_and_marker_identity \
  triggers::postgres_platform_login_connection_insert_side_effects_never_publish_or_consume_otp \
  triggers::postgres_platform_login_connection_deferred_insert_side_effects_are_flushed_before_commit \
  waits::postgres_platform_login_connection_rechecks_original_catalog_after_observed_source_wait \
  waits::postgres_platform_login_connection_cancelled_source_wait_closes_connection_without_otp_write; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_login_connection_tdd "$platform_login_case" -- --ignored --list | grep -Fx "$platform_login_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_login_connection_tdd "$platform_login_case" -- --ignored --exact
done

for platform_login_gap_case in \
  postgres_platform_login_gap_cancellation_keeps_real_password_job_capacity_without_source_resources \
  postgres_platform_login_gap_rejects_original_root_marker_and_namespace_drift_without_otp_or_session_writes \
  postgres_platform_login_gap_cannot_adopt_an_account_created_after_initial_absence; do
  platform_login_selector="services::auth_service::durable_source::password_work::platform_login_gap_sql_tests::$platform_login_gap_case"
  cargo test --manifest-path engine/Cargo.toml --locked --lib "$platform_login_selector" -- --ignored --list | grep -Fx "$platform_login_selector: test"
  cargo test --manifest-path engine/Cargo.toml --locked --lib "$platform_login_selector" -- --ignored --exact
done
