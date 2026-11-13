#!/usr/bin/env bash
set -euo pipefail
# This entry runs only inside a newly created disposable test container.
[[ $# == 4 && ( "$1" == preflight || "$1" == tls ) ]] || exit 2
browser_entry_mode="$1"
browser_entry_repo="$2"
browser_entry_session="$3"
browser_entry_body="$4"
[[ "$(id -u)" == 1000 && "$(id -g)" == 1000 && "$(id -un)" == ubuntu ]] || exit 2
browser_entry_account_home="$(getent passwd ubuntu | cut -d: -f6)"
[[ "$HOME" == "$browser_entry_account_home" && "$HOME" == /home/ubuntu ]] || exit 2
[[ ! -e "$HOME/.pki/nssdb" && ! -L "$HOME/.pki/nssdb" ]] || {
  printf 'Container default account has legacy NSS; refusing TLS tests.\n' >&2; exit 2;
}
for browser_entry_field in CapPrm: CapEff: CapInh: CapAmb: CapBnd:; do
  [[ "$(awk -v field="$browser_entry_field" '$1 == field {print $2}' /proc/self/status)" == 0000000000000000 ]] || exit 2
done
[[ "$(awk '$1 == "NoNewPrivs:" {print $2}' /proc/self/status)" == 1 ]] || exit 2
export PATH="/opt/cyanrex-browser/node/bin:/usr/lib/postgresql/16/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
export CYANREX_PLAYWRIGHT_MODULE=/opt/cyanrex-browser/node_modules/playwright/index.mjs
export CYANREX_CHROMIUM_PATH=/opt/google/chrome/chrome
for browser_entry_tool in node openssl certutil initdb pg_ctl psql timeout; do command -v "$browser_entry_tool" >/dev/null; done
browser_entry_node_version="$(node --version)"
[[ "$browser_entry_node_version" =~ ^v22\.[0-9]+\.[0-9]+$ ]] || exit 2
[[ "$("$CYANREX_CHROMIUM_PATH" --version)" =~ ([0-9]+)\.([0-9]+)\.([0-9]+)\.([0-9]+) ]] || exit 2
(( BASH_REMATCH[1] >= 146 )) || exit 2
browser_entry_chrome_version="${BASH_REMATCH[0]}"
[[ "$(pg_ctl --version)" =~ ([0-9]+)\.([0-9]+) ]] || exit 2
browser_entry_pg_version="${BASH_REMATCH[0]}"
printf 'Container runtime: Node %s; Chrome %s; PostgreSQL %s.\n' \
  "$browser_entry_node_version" "$browser_entry_chrome_version" "$browser_entry_pg_version"
for browser_entry_binary in "$browser_entry_session" "$browser_entry_body" "$browser_entry_repo/engine/target/debug/cyanrex-platform"; do
  [[ -x "$browser_entry_binary" ]] || exit 2
  browser_entry_links="$(ldd "$browser_entry_binary")"
  [[ "$browser_entry_links" != *'not found'* ]] || exit 2
done
browser_entry_session_case=tls::postgres_platform_browser_tls_private_ca_real_cookie_session_and_storage
browser_entry_body_case=behavior::postgres_platform_browser_task_body_tls_confirmed_replace_conflict_and_revocation
"$browser_entry_session" "$browser_entry_session_case" --ignored --list | grep -Fx "$browser_entry_session_case: test"
"$browser_entry_body" "$browser_entry_body_case" --ignored --list | grep -Fx "$browser_entry_body_case: test"
browser_entry_private="$(mktemp -d /tmp/cyanrex-browser-container-XXXXXX)"
chmod 700 "$browser_entry_private"
browser_entry_started=false
cleanup() {
  local outcome=$?
  trap - EXIT
  if [[ "$browser_entry_started" == true ]]; then
    if pg_ctl -D "$browser_entry_private/pgdata" -m fast -w stop >/dev/null \
      && [[ ! -e "$browser_entry_private/pgdata/postmaster.pid" ]]; then
      printf 'Disposable PostgreSQL stopped; no host database or trust was used.\n'
    else
      printf 'Disposable PostgreSQL stop was not confirmed.\n' >&2; outcome=1
    fi
  fi
  exit "$outcome"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
mkdir -m 700 "$browser_entry_private/data" "$browser_entry_private/config" "$browser_entry_private/cache"
export XDG_DATA_HOME="$browser_entry_private/data"
export XDG_CONFIG_HOME="$browser_entry_private/config"
export XDG_CACHE_HOME="$browser_entry_private/cache"
timeout 40s node --input-type=module -e '
  import { pathToFileURL } from "node:url";
  const { chromium } = await import(pathToFileURL(process.env.CYANREX_PLAYWRIGHT_MODULE).href);
  const browser = await chromium.launch({ executablePath: process.env.CYANREX_CHROMIUM_PATH, headless: true });
  await browser.close();
'
printf 'Container preflight passed: image account/default HOME, zero capabilities, Node22, Chrome and native test ABI.\n'
[[ "$browser_entry_mode" == tls ]] || exit 0
printf 'synthetic-disposable-browser-admin\n' > "$browser_entry_private/admin.password"
chmod 600 "$browser_entry_private/admin.password"
initdb -D "$browser_entry_private/pgdata" --no-locale --encoding=UTF8 --auth-local=trust \
  --auth-host=scram-sha-256 --username=cyanrex_browser_admin \
  --pwfile="$browser_entry_private/admin.password" >/dev/null
pg_ctl -D "$browser_entry_private/pgdata" \
  -o "-h 127.0.0.1 -p 25432 -k $browser_entry_private" \
  -l "$browser_entry_private/postgresql.log" -w start >/dev/null
browser_entry_started=true
export CYANREX_TEST_DATABASE_URL='postgres://cyanrex_browser_admin:synthetic-disposable-browser-admin@127.0.0.1:25432/postgres'
cd "$browser_entry_repo"
timeout 180s "$browser_entry_session" "$browser_entry_session_case" --ignored --exact
timeout 180s "$browser_entry_body" "$browser_entry_body_case" --ignored --exact
browser_entry_remaining="$(psql --no-psqlrc "$CYANREX_TEST_DATABASE_URL" -Atc "SELECT (SELECT count(*) FROM pg_database WHERE datname NOT IN ('postgres','template0','template1')) || ':' || (SELECT count(*) FROM pg_roles WHERE rolname NOT LIKE 'pg_%') || ':' || (SELECT count(*) FROM pg_stat_activity WHERE backend_type = 'client backend' AND pid <> pg_backend_pid())")"
[[ "$browser_entry_remaining" == 0:1:0 ]] || {
  printf 'Disposable database cleanup observation failed.\n' >&2; exit 1;
}
printf 'Both original TLS selectors passed; only initial SQL databases/administrator remain, with zero other clients.\n'
