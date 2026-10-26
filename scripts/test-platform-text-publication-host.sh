#!/usr/bin/env bash
set -euo pipefail

# Six exact workspace/plain/native-TLS boundaries; caller owns the disposable target.
: "${CYANREX_TEST_DATABASE_URL:?Set an explicit disposable PostgreSQL database URL}"
text_publication_host_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$text_publication_host_root"

for text_publication_host_case in \
  behavior::postgres_platform_text_publication_workspace_explicit_chain_and_old_constructors_stay_compatible \
  boundaries::postgres_platform_text_publication_workspace_original_binding_and_current_authority_fail_closed \
  capacity::postgres_platform_text_publication_workspace_seven_independent_response_lanes_share_clones \
  children::postgres_platform_text_publication_plain_child_login_publish_attach_reopen_and_shutdown \
  children::postgres_platform_text_publication_tls_child_ca_ip_san_cookie_publish_and_shutdown \
  permissions::postgres_platform_text_publication_workspace_minimal_grants_and_unknown_orphans_are_not_repaired; do
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_text_publication_host_tdd "$text_publication_host_case" -- --ignored --list | grep -Fx "$text_publication_host_case: test"
  cargo test --manifest-path engine/Cargo.toml --locked --test platform_text_publication_host_tdd "$text_publication_host_case" -- --ignored --exact
done
