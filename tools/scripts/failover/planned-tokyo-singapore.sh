#!/usr/bin/env bash
set -euo pipefail

: "${ANNOUNCE_MAINTENANCE_COMMAND:?ANNOUNCE_MAINTENANCE_COMMAND is required}"
: "${SET_DNS_TTL_60_COMMAND:?SET_DNS_TTL_60_COMMAND is required}"
: "${DNS_SHIFT_COMMAND:?DNS_SHIFT_COMMAND is required}"
: "${DB_FAILOVER_COMMAND:?DB_FAILOVER_COMMAND is required}"
: "${CANARY_COMMAND:?CANARY_COMMAND is required}"
: "${ROLLBACK_COMMAND:?ROLLBACK_COMMAND is required}"

eval "${ANNOUNCE_MAINTENANCE_COMMAND}"
eval "${SET_DNS_TTL_60_COMMAND}"

rollback() {
  echo "planned failover failed; running rollback" >&2
  eval "${ROLLBACK_COMMAND}"
}
trap rollback ERR

eval "${DNS_SHIFT_COMMAND}"
eval "${DB_FAILOVER_COMMAND}"
eval "${CANARY_COMMAND}"

trap - ERR
echo "planned tokyo -> singapore failover completed"
