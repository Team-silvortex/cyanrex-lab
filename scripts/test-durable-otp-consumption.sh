#!/usr/bin/env bash
set -euo pipefail

# Real transactions with private test-only clocks; never a deployed database or public clock API.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
otp_consumption_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$otp_consumption_repo_root"
otp_consumption_prefix=services::auth_service::durable_source::otp::consumption_sql_tests

for otp_consumption_case in \
  postgres_otp_consumption_independent_sources_cannot_both_use_one_code \
  behavior::postgres_otp_consumption_survives_reopen_logout_and_allows_only_new_steps \
  behavior::postgres_otp_consumption_login_and_rotation_compete_for_the_same_step \
  behavior::postgres_otp_consumption_is_scoped_to_exact_accounts_and_new_incarnations \
  behavior::postgres_otp_consumption_rejects_version_one_without_adoption_or_writes \
  behavior::postgres_otp_consumption_rejects_incompatible_counter_shape_without_repair \
  faults::postgres_otp_consumption_suppressed_writes_roll_back_and_leave_the_code_retryable \
  faults::postgres_otp_consumption_rechecks_counter_after_update_and_session_trigger_tampering \
  faults::postgres_otp_consumption_deferred_commit_failure_rolls_back_the_counter_and_effects \
  faults::postgres_otp_consumption_cancellation_at_post_write_wait_rolls_back_pending_consumption \
  faults::postgres_otp_consumption_real_collision_pins_highest_step_and_cannot_retarget_at_commit; do
  otp_consumption_name="$otp_consumption_prefix::$otp_consumption_case"
  cargo test --manifest-path engine/Cargo.toml --locked --lib "$otp_consumption_name" -- --ignored --list | grep -Fx "$otp_consumption_name: test"
  cargo test --manifest-path engine/Cargo.toml --locked --lib "$otp_consumption_name" -- --ignored --exact --nocapture
done
