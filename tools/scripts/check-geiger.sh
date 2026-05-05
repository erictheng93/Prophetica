#!/usr/bin/env bash
set -euo pipefail

file="${1:?geiger json path required}"

if [ ! -s "$file" ]; then
  printf 'geiger output missing or empty: %s\n' "$file" >&2
  exit 1
fi

if grep -Eq '"functions"[[:space:]]*:[[:space:]]*[1-9]|"exprs"[[:space:]]*:[[:space:]]*[1-9]|"item_impls"[[:space:]]*:[[:space:]]*[1-9]' "$file"; then
  printf 'unsafe usage detected by cargo geiger\n' >&2
  exit 1
fi

printf 'cargo geiger unsafe usage check passed\n'
