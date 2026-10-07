#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable PostgreSQL only; never infer a deployment connection.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
resource_sql_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$resource_sql_repo_root"

for resource_sql_case in \
  services::prepared_resource_sql::tests::postgres_ordinary_table_predicates_and_decode_order_are_preserved \
  services::prepared_resource_sql::tests::postgres_transaction_modes_and_local_limits_do_not_leak_after_rollback; do
  cargo test --manifest-path engine/Cargo.toml --locked --lib "$resource_sql_case" -- --ignored --list | grep -Fx "$resource_sql_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --lib "$resource_sql_case" -- --ignored --exact --nocapture
done
