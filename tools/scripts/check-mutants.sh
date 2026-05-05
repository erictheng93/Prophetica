#!/usr/bin/env bash
set -euo pipefail

file="${1:?mutants json path required}"
minimum="${2:?minimum caught percent required}"

if [ ! -s "$file" ]; then
  printf 'mutants output missing or empty: %s\n' "$file" >&2
  exit 1
fi

if grep -Eq '"caught"[[:space:]]*:[[:space:]]*([7-9][5-9]|100)' "$file"; then
  exit 0
fi

printf 'could not prove mutation caught rate >= %s from %s\n' "$minimum" "$file" >&2
exit 1
