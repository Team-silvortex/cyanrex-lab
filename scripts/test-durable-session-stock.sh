#!/usr/bin/env bash
set -euo pipefail

# Synthetic transactions and private clocks belong only to an explicit disposable database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
session_stock_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$session_stock_repo_root"
session_stock_prefix=services::auth_service::durable_source::session_stock_sql_tests

for session_stock_case in \
  behavior::postgres_session_stock_full_account_preserves_expired_device_and_unused_otp_until_explicit_logout \
  behavior::postgres_session_stock_total_counts_other_accounts_without_consuming_the_requesting_accounts_otp \
  behavior::postgres_session_stock_policy_construction_does_not_change_the_legacy_login_entry_point \
  behavior::postgres_session_stock_counts_expired_infinite_and_out_of_chrono_range_rows_without_decoding_or_cleanup \
  faults::postgres_session_stock_immediate_and_deferred_extra_rows_roll_back_even_below_configured_limits \
  faults::postgres_session_stock_original_source_drift_and_deferred_constraint_failure_never_publish_a_session \
  concurrency::postgres_session_stock_independent_repeatable_read_sources_compete_for_the_last_persisted_session \
  concurrency::postgres_session_stock_actual_post_insert_cancellation_preserves_stock_and_otp_after_observed_rollback; do
  session_stock_name="$session_stock_prefix::$session_stock_case"
  cargo test --manifest-path engine/Cargo.toml --locked --lib "$session_stock_name" -- --ignored --list | grep -Fx "$session_stock_name: test"
  cargo test --manifest-path engine/Cargo.toml --locked --lib "$session_stock_name" -- --ignored --exact
done
