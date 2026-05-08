#!/usr/bin/env bash
set -euo pipefail

: "${RUNBOOK_PATH:=docs/runbooks/dr-failover.md}"
: "${CANARY_COMMAND:?CANARY_COMMAND is required}"

echo "Loading runbook: ${RUNBOOK_PATH}"
test -f "${RUNBOOK_PATH}"
eval "${CANARY_COMMAND}"
echo "Tokyo -> Singapore failover canary completed"
