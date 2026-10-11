#!/usr/bin/env bash
set -euo pipefail

# Trusted intent/quota fixtures require a disposable database chosen explicitly by the caller.
# Never fall back to DATABASE_URL, a live Engine database, authentication, or any file-staging root.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
upload_intent_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$upload_intent_repo_root"

for upload_intent_pg_case in \
  baseline::postgres_first_reservation_persists_and_reopens \
  cases::persistence::postgres_duplicate_upload_and_file_version_are_conflicts_before_quota \
  cases::binding::postgres_owner_generation_scope_and_complete_manifest_binding_are_exact \
  cases::quota::postgres_byte_limits_apply_globally_and_per_principal_across_generations \
  cases::quota::postgres_empty_files_consume_upload_counts_without_reserving_bytes \
  cases::concurrency::postgres_concurrent_repeatable_read_defaults_cannot_oversubscribe_quota \
  cases::concurrency::postgres_cancelled_real_reservation_wait_rolls_back_and_restores_pool \
  cases::records::postgres_corrupt_foreign_ledger_and_oversized_manifests_fail_closed_without_writes \
  cases::schema::postgres_installation_limits_scope_and_schema_shape_pins_are_not_adopted \
  cases::schema::postgres_query_shadow_and_in_transaction_relation_replacement_are_rejected \
  cases::faults::postgres_postwrite_foreign_mutations_and_unexpected_deltas_never_confirm \
  cases::faults::postgres_insert_and_deferred_commit_faults_preserve_prior_reservations; do
  cargo test --manifest-path engine/Cargo.toml --locked --test file_upload_intent_tdd \
    "$upload_intent_pg_case" -- --ignored --list | grep -Fx -- "$upload_intent_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test file_upload_intent_tdd \
    "$upload_intent_pg_case" -- --ignored --exact --nocapture
done
