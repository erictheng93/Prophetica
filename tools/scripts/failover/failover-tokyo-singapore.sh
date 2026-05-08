#!/usr/bin/env bash
set -euo pipefail

: "${TOKYO_EDGE_HEALTH_COMMAND:?TOKYO_EDGE_HEALTH_COMMAND is required}"
: "${TOKYO_DB_HEALTH_COMMAND:?TOKYO_DB_HEALTH_COMMAND is required}"
: "${SINGAPORE_DR_READY_COMMAND:?SINGAPORE_DR_READY_COMMAND is required}"
: "${KAFKA_MM2_LAG_CHECK_COMMAND:?KAFKA_MM2_LAG_CHECK_COMMAND is required}"
: "${DNS_SHIFT_COMMAND:?DNS_SHIFT_COMMAND is required}"
: "${DB_FAILOVER_COMMAND:?DB_FAILOVER_COMMAND is required}"
: "${SINGAPORE_ENGINE_START_COMMAND:?SINGAPORE_ENGINE_START_COMMAND is required}"
: "${CANARY_COMMAND:?CANARY_COMMAND is required}"
: "${ROLLBACK_COMMAND:?ROLLBACK_COMMAND is required}"

tokyo_edge_ok=0
tokyo_db_ok=0
if eval "${TOKYO_EDGE_HEALTH_COMMAND}"; then tokyo_edge_ok=1; fi
if eval "${TOKYO_DB_HEALTH_COMMAND}"; then tokyo_db_ok=1; fi

if [ "${tokyo_edge_ok}" -eq 1 ] && [ "${tokyo_db_ok}" -eq 1 ]; then
  echo "Tokyo is still healthy; refusing disaster failover" >&2
  exit 3
fi

eval "${SINGAPORE_DR_READY_COMMAND}"
eval "${KAFKA_MM2_LAG_CHECK_COMMAND}"

rollback() {
  echo "failover failed; running rollback" >&2
  eval "${ROLLBACK_COMMAND}"
}
trap rollback ERR

eval "${DNS_SHIFT_COMMAND}"
eval "${DB_FAILOVER_COMMAND}"
eval "${SINGAPORE_ENGINE_START_COMMAND}"
eval "${CANARY_COMMAND}"

trap - ERR
echo "tokyo -> singapore failover completed"
