#!/usr/bin/env bash
set -euo pipefail

direction="${1:-up}"
case "$direction" in
  up)
    if find services -path '*/migrations/*' -type f -name '*.sql' | grep -q .; then
      sqlx migrate run --source services/wallet-ledger-service/migrations
    else
      printf 'no SQL migrations present in bootstrap scaffold\n'
    fi
    ;;
  *)
    printf 'unsupported migration direction: %s\n' "$direction" >&2
    exit 2
    ;;
esac
