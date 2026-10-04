#!/usr/bin/env bash
set -euo pipefail

# Private test clocks are compiled only into the library's test binary; never a public clock API.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
otp_freshness_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$otp_freshness_repo_root"
otp_freshness_prefix=services::auth_service::durable_source::otp::sql_tests

for otp_freshness_case in \
  postgres_otp_login_rechecks_freshness_after_source_writer_wait \
  postgres_otp_login_rechecks_freshness_after_session_insert_wait \
  postgres_otp_rotation_rechecks_freshness_after_session_revoke_wait \
  postgres_otp_unchanged_clock_preserves_success_through_writer_and_trigger_waits \
  postgres_otp_rotation_retains_its_existing_writer_wait_freshness_check \
  postgres_otp_login_rechecks_freshness_while_password_executor_is_held \
  postgres_otp_freshness_does_not_claim_single_use_across_independent_sources; do
  otp_freshness_name="$otp_freshness_prefix::$otp_freshness_case"
  cargo test --manifest-path engine/Cargo.toml --locked --lib "$otp_freshness_name" -- --ignored --list | grep -Fx "$otp_freshness_name: test"
  cargo test --manifest-path engine/Cargo.toml --locked --lib "$otp_freshness_name" -- --ignored --exact --nocapture
done
