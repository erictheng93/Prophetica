#!/usr/bin/env bash
set -euo pipefail

: "${SINGAPORE_EDGE_HEALTH_URL:?SINGAPORE_EDGE_HEALTH_URL is required}"
: "${SINGAPORE_DATABASE_URL:?SINGAPORE_DATABASE_URL is required}"
: "${SINGAPORE_ENGINE_HEALTH_URL:?SINGAPORE_ENGINE_HEALTH_URL is required}"
: "${SINGAPORE_WALLET_HEALTH_URL:?SINGAPORE_WALLET_HEALTH_URL is required}"
: "${SINGAPORE_ORDER_BOOK_URL:?SINGAPORE_ORDER_BOOK_URL is required}"
: "${THIRD_PARTY_FALLBACK_HEALTH_COMMAND:?THIRD_PARTY_FALLBACK_HEALTH_COMMAND is required}"
: "${ROLLBACK_COMMAND:?ROLLBACK_COMMAND is required}"

rollback() {
  echo "canary failed; running rollback" >&2
  eval "${ROLLBACK_COMMAND}"
}
trap rollback ERR

curl -fsS "${SINGAPORE_EDGE_HEALTH_URL}" >/dev/null
psql "${SINGAPORE_DATABASE_URL}" --command "SELECT 1" >/dev/null
curl -fsS "${SINGAPORE_ENGINE_HEALTH_URL}" >/dev/null
curl -fsS "${SINGAPORE_WALLET_HEALTH_URL}" >/dev/null
curl -fsS "${SINGAPORE_ORDER_BOOK_URL}" >/dev/null
eval "${THIRD_PARTY_FALLBACK_HEALTH_COMMAND}"

trap - ERR
echo "singapore canary passed"
