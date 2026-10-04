#!/usr/bin/env bash
set -euo pipefail

# Credential fault injection requires an explicitly selected disposable database.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
password_profile_repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$password_profile_repo_root"

for password_profile_case in \
  postgres_password_profile_login_rejects_unsupported_records_without_issuing_sessions \
  postgres_password_profile_existing_session_is_not_implicitly_revoked_and_can_logout \
  postgres_password_profile_rotation_rejects_bad_record_without_changing_session_or_registry \
  postgres_password_profile_official_registration_rotation_and_bootstrap_remain_usable \
  postgres_password_profile_registration_rechecks_trigger_changed_profile_before_commit \
  postgres_password_profile_rotation_rechecks_post_revoke_profile_and_rolls_back; do
  cargo test --manifest-path engine/Cargo.toml --locked --test durable_password_profile_tdd "$password_profile_case" -- --ignored --list | grep -Fx "$password_profile_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test durable_password_profile_tdd "$password_profile_case" -- --ignored --exact --nocapture
done
