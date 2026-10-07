#!/usr/bin/env bash
set -euo pipefail

# Shared worker admission evidence on synthetic data, not a timing-equivalence benchmark.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
login_password_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$login_password_repo_root"
login_password_prefix=services::auth_service::durable_source::password_work::login_sql_tests

for login_password_case in \
  postgres_login_password_missing_account_queues_work_without_holding_source_resources \
  behavior::postgres_login_password_matching_public_dummy_still_cannot_authenticate_or_reach_otp \
  behavior::postgres_login_password_known_wrong_credentials_take_the_same_bounded_worker_path \
  behavior::postgres_login_password_account_created_in_worker_gap_cannot_replace_initial_absence \
  behavior::postgres_login_password_global_admission_applies_to_existing_and_missing_accounts \
  behavior::postgres_login_password_input_profile_and_source_errors_are_not_dummy_fallbacks \
  behavior::postgres_login_password_missing_denials_retain_the_per_username_attempt_budget; do
  login_password_name="$login_password_prefix::$login_password_case"
  cargo test --manifest-path engine/Cargo.toml --locked --lib "$login_password_name" -- --ignored --list | grep -Fx "$login_password_name: test"
  cargo test --manifest-path engine/Cargo.toml --locked --lib "$login_password_name" -- --ignored --exact --nocapture
done
