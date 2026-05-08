#!/usr/bin/env bash
set -euo pipefail

: "${FALLBACK_NAME:?FALLBACK_NAME is required}"
: "${TARGET_HEALTH_CHECK_COMMAND:?TARGET_HEALTH_CHECK_COMMAND is required}"
: "${SWITCH_COMMAND:?SWITCH_COMMAND is required}"
: "${POST_SWITCH_HEALTH_CHECK_COMMAND:?POST_SWITCH_HEALTH_CHECK_COMMAND is required}"
: "${ROLLBACK_COMMAND:?ROLLBACK_COMMAND is required}"

eval "${TARGET_HEALTH_CHECK_COMMAND}"

rollback() {
  echo "${FALLBACK_NAME} fallback failed; running rollback" >&2
  eval "${ROLLBACK_COMMAND}"
}
trap rollback ERR

eval "${SWITCH_COMMAND}"
eval "${POST_SWITCH_HEALTH_CHECK_COMMAND}"

trap - ERR
echo "${FALLBACK_NAME} fallback switched successfully"
