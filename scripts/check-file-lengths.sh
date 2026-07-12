#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
status=0

while IFS= read -r -d '' file; do
  relative="${file#"$root/"}"
  case "$relative" in
    .git/*|*/node_modules/*|*/public/monaco/*|*/target/*|*/.next/*|*/dist/*|*/build/*|\
    */Cargo.lock|*/package-lock.json|*/tsconfig.tsbuildinfo)
      continue
      ;;
  esac

  lines=$(wc -l < "$file")
  case "$file" in
    *.md|*.mdx|*.rst)
      limit=2000
      ;;
    *.rs|*.ts|*.tsx|*.js|*.jsx|*.mjs|*.c|*.h|*.cpp|*.hpp|*.py|*.sh|*.css|*.scss)
      limit=600
      ;;
    *)
      continue
      ;;
  esac

  if (( lines > limit )); then
    printf '%s: %d lines (limit: %d)\n' "$relative" "$lines" "$limit" >&2
    status=1
  fi
done < <(find "$root" -type f -print0)

if (( status == 0 )); then
  echo "File length limits passed."
fi

exit "$status"
