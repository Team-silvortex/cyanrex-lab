#!/usr/bin/env bash
set -euo pipefail

# Explicit disposable PostgreSQL only; no ambient database or live installation.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
sharing_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$sharing_repo_root"

for sharing_pg_case in \
  baseline::postgres_session_publisher_shares_exact_published_version_and_revokes_after_publisher_revocation \
  behavior::postgres_session_sharing_self_and_trusted_issuer_remain_owned_without_files_or_installer_pools \
  concurrency::postgres_session_sharing_queued_writers_have_single_quota_and_cas_winners_with_fresh_defaults \
  faults::postgres_session_sharing_immediate_and_deferred_authority_and_record_faults_roll_back \
  grants::postgres_session_sharing_requires_current_own_active_publisher_permission_but_not_for_revoke \
  lifecycle::postgres_session_sharing_rechecks_current_actor_and_target_but_revokes_retired_recipients \
  relations::postgres_session_sharing_original_source_registry_publication_and_grant_relations_are_not_adopted \
  schema::postgres_session_sharing_rejects_namespace_scope_schema_and_incarnation_drift \
  selectors::postgres_session_sharing_compares_all_published_coordinates_and_exact_owner_generations \
  waits::postgres_session_sharing_real_waits_recheck_expiry_and_cancel_before_commit_with_rollback_observation; do
  cargo test --manifest-path engine/Cargo.toml --locked --test session_file_sharing_tdd \
    "$sharing_pg_case" -- --ignored --list | grep -Fx "$sharing_pg_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test session_file_sharing_tdd \
    "$sharing_pg_case" -- --ignored --exact --nocapture
done
