#!/usr/bin/env bash
set -euo pipefail

: "${DATABASE_URL:?DATABASE_URL is required}"

psql "${DATABASE_URL}" \
  --tuples-only \
  --no-align \
  --command "SELECT state || ',' || sync_state FROM pg_stat_replication WHERE application_name = 'standby1'" \
  | grep -qx 'streaming,sync'
