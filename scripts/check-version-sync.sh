#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATUS=0

read_json_version() {
  awk -F'"' '/"version"[[:space:]]*:/ {print $4; exit}' "$1"
}

read_cargo_lock_version() {
  awk '
    $0 == "name = \"cyanrex-engine\"" { found = 1; next }
    found && /^version = / {
      gsub(/^version = \"|\"$/, "")
      print
      exit
    }
  ' "$1"
}

assert_version() {
  local label="$1"
  local actual="$2"
  local expected="$3"
  if [[ "$actual" != "$expected" ]]; then
    printf 'Version mismatch: %s is %s, expected %s\n' "$label" "${actual:-<missing>}" "$expected" >&2
    STATUS=1
  fi
}

assert_contains_version() {
  local file="$1"
  local expected="$2"
  if ! grep -Fq "$expected" "$file"; then
    printf 'Version mismatch: %s does not reference %s\n' "${file#"$PROJECT_ROOT/"}" "$expected" >&2
    STATUS=1
  fi
}

CANONICAL_VERSION="$(awk -F'"' '/^version = / {print $2; exit}' "$PROJECT_ROOT/engine/Cargo.toml")"
if [[ ! "$CANONICAL_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Invalid canonical version in engine/Cargo.toml: ${CANONICAL_VERSION:-<missing>}" >&2
  exit 1
fi

README_VERSION="$(sed -n 's/^Version: `\([^`]*\)`.*/\1/p' "$PROJECT_ROOT/README.md" | head -n 1)"
assert_version "README.md" "$README_VERSION" "$CANONICAL_VERSION"
assert_version "engine/Cargo.lock" "$(read_cargo_lock_version "$PROJECT_ROOT/engine/Cargo.lock")" "$CANONICAL_VERSION"
assert_version "frontend/package.json" "$(read_json_version "$PROJECT_ROOT/frontend/package.json")" "$CANONICAL_VERSION"
assert_version "frontend/package-lock.json" "$(read_json_version "$PROJECT_ROOT/frontend/package-lock.json")" "$CANONICAL_VERSION"
assert_version "sdk-js/package.json" "$(read_json_version "$PROJECT_ROOT/sdk-js/package.json")" "$CANONICAL_VERSION"
assert_version "sdk-js/package-lock.json" "$(read_json_version "$PROJECT_ROOT/sdk-js/package-lock.json")" "$CANONICAL_VERSION"

for versioned_doc in \
  "$PROJECT_ROOT/docs/en/runner-agent.md" \
  "$PROJECT_ROOT/docs/zh-CN/runner-agent.md" \
  "$PROJECT_ROOT/frontend/public/course/en/runner-agent.md" \
  "$PROJECT_ROOT/frontend/public/course/zh-CN/runner-agent.md"; do
  assert_contains_version "$versioned_doc" "$CANONICAL_VERSION"
done

if (( STATUS != 0 )); then
  exit "$STATUS"
fi

echo "Version metadata is synchronized at ${CANONICAL_VERSION}."
