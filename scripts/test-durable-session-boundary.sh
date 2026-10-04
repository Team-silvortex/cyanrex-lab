#!/usr/bin/env bash
set -euo pipefail

# Namespace/trigger/relation fault injection is restricted to an explicit disposable database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
session_boundary_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$session_boundary_repo_root"

for session_boundary_pg_case in \
  postgres_session_boundary_logout_cannot_confirm_against_redirected_empty_sessions \
  shapes::postgres_session_boundary_normal_source_needs_no_collaboration_registry \
  shapes::postgres_session_boundary_all_entry_points_reject_ambiguous_search_path \
  shapes::postgres_session_boundary_all_entry_points_reject_temporary_relation_shadowing \
  shapes::postgres_session_boundary_reads_reject_views_row_security_and_unknown_storage \
  shapes::postgres_session_boundary_unprepared_or_wrong_authority_never_installs_or_adopts \
  writes::postgres_session_boundary_login_rechecks_source_metadata_after_insert \
  writes::postgres_session_boundary_logout_rechecks_source_metadata_after_delete \
  writes::postgres_session_boundary_login_rejects_identical_replacement_account_relation \
  writes::postgres_session_boundary_login_rejects_a_fully_valid_redirected_namespace \
  writes::postgres_session_boundary_logout_rejects_replaced_source_relation_and_restores_session \
  concurrency::postgres_session_boundary_validate_rechecks_expiry_after_source_lock_wait \
  concurrency::postgres_session_boundary_validate_waits_for_exact_logout_commit \
  concurrency::postgres_session_boundary_login_retains_its_first_namespace_pin_across_password_work; do
  cargo test --manifest-path engine/Cargo.toml --locked --test durable_session_boundary_tdd "$session_boundary_pg_case" -- --ignored --list | grep -Fx "$session_boundary_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test durable_session_boundary_tdd "$session_boundary_pg_case" -- --ignored --exact --nocapture
done
