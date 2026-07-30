#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EXCEPTIONS_FILE="$PROJECT_ROOT/scripts/security-audit-exceptions.json"
LOCK_FILE="$PROJECT_ROOT/engine/Cargo.lock"

if [[ ! -f "$LOCK_FILE" ]]; then
  echo "Missing lock file: $LOCK_FILE" >&2
  exit 1
fi

if [[ ! -f "$EXCEPTIONS_FILE" ]]; then
  echo "Missing exception registry: $EXCEPTIONS_FILE" >&2
  exit 1
fi

tmp_report="$(mktemp)"
trap 'rm -f "$tmp_report"' EXIT

set +e
cargo audit --json --file "$LOCK_FILE" > "$tmp_report"
audit_exit=$?
set -e

if [[ "$audit_exit" -gt 1 ]]; then
  echo "cargo audit failed before returning a parseable report." >&2
  exit "$audit_exit"
fi

python3 - "$tmp_report" "$EXCEPTIONS_FILE" <<'PY'
import datetime
import json
import sys

report_path = sys.argv[1]
exceptions_path = sys.argv[2]

with open(report_path, 'r', encoding='utf-8') as f:
    report = json.load(f)

with open(exceptions_path, 'r', encoding='utf-8') as f:
    exceptions = json.load(f)

accepted = {item['id']: item for item in exceptions.get('ignore', [])}
today = datetime.date.today()

vulns = (report.get('vulnerabilities') or {}).get('list', [])
if not vulns:
    print('✅ cargo audit reported no vulnerabilities.')
    sys.exit(0)

unaccepted = []

for vuln in vulns:
    adv = vuln.get('advisory', {})
    adv_id = adv.get('id', '<unknown>')
    package_info = adv.get('package')
    package = package_info.get('name') if isinstance(package_info, dict) else package_info
    if package is None:
        package = '<unknown>'
    title = adv.get('title', 'No title')

    override_entry = accepted.get(adv_id)
    if override_entry is None:
        unaccepted.append((adv_id, package, title, vuln))
        continue

    review_by = override_entry.get('review_by')
    if review_by:
        expiry = datetime.date.fromisoformat(review_by)
        if today > expiry:
            unaccepted.append((adv_id, package, f"Accepted-risk expired on {review_by}", vuln))
            continue

    print(f"⚠️  tolerated advisory: {adv_id} ({package})")

if unaccepted:
    print('🚫 Unaccepted security advisories detected:')
    for adv_id, package, title, _ in unaccepted:
        print(f"  - {adv_id} in {package}: {title}")
    print()
    print(f"Exceptions file: {sys.argv[2]}")
    print('Please update exception registry or upgrade dependencies to remove these advisories.')
    sys.exit(1)

print('✅ Security audit passed with only tracked accepted advisories.')
PY
