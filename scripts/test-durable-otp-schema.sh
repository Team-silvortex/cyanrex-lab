#!/usr/bin/env bash
set -euo pipefail

# Explicit opt-in: schema/credential fault tests run only on a disposable PostgreSQL target.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
otp_schema_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$otp_schema_repo_root"

for otp_schema_case in \
  postgres_otp_schema_one_is_rejected_without_adoption_or_repair \
  postgres_otp_schema_two_requires_exact_counter_shape_without_repair \
  postgres_otp_schema_fresh_registration_and_bootstrap_start_unconsumed \
  postgres_otp_schema_initial_counter_faults_are_rechecked_and_rolled_back; do
  cargo test --manifest-path engine/Cargo.toml --locked --test durable_otp_schema_tdd "$otp_schema_case" -- --ignored --list | grep -Fx "$otp_schema_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test durable_otp_schema_tdd "$otp_schema_case" -- --ignored --exact --nocapture
done
