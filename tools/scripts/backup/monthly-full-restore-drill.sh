#!/usr/bin/env bash
set -euo pipefail

: "${LATEST_BASEBACKUP_ARCHIVE:?LATEST_BASEBACKUP_ARCHIVE is required}"
: "${RESTORE_TARGET_TIME:?RESTORE_TARGET_TIME is required}"
: "${RESTORE_PGDATA:=/tmp/prophetica-monthly-full-restore}"
: "${R2_WAL_ARCHIVE_URI:?R2_WAL_ARCHIVE_URI is required}"
: "${RESTORE_VERIFY_DATABASE_URL:?RESTORE_VERIFY_DATABASE_URL is required}"

BASEBACKUP_ARCHIVE="${LATEST_BASEBACKUP_ARCHIVE}" \
RESTORE_PGDATA="${RESTORE_PGDATA}" \
RESTORE_TARGET_TIME="${RESTORE_TARGET_TIME}" \
R2_WAL_ARCHIVE_URI="${R2_WAL_ARCHIVE_URI}" \
  "$(dirname "$0")/../postgres/pitr-restore.sh"

psql "${RESTORE_VERIFY_DATABASE_URL}" --command "SELECT COUNT(*) FROM ledger_entries" >/dev/null
echo "monthly full restore drill prepared and ledger check query passed"
