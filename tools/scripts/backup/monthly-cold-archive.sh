#!/usr/bin/env bash
set -euo pipefail

: "${DATABASE_URL:?DATABASE_URL is required}"
: "${COLD_ARCHIVE_WORKDIR:=/var/backups/prophetica/cold-archive}"
: "${R2_OBJECT_LOCK_URI:?R2_OBJECT_LOCK_URI is required}"
: "${GLACIER_ARCHIVE_URI:?GLACIER_ARCHIVE_URI is required}"
: "${COLD_ARCHIVE_RETENTION_YEARS:=7}"

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
dump_path="${COLD_ARCHIVE_WORKDIR}/ledger-critical-${stamp}.dump"

mkdir -p "${COLD_ARCHIVE_WORKDIR}"
pg_dump "${DATABASE_URL}" \
  --format=custom \
  --file="${dump_path}" \
  --table=ledger_entries \
  --table=ledger_transactions \
  --table=settlements

rclone copyto "${dump_path}" "${R2_OBJECT_LOCK_URI%/}/$(basename "${dump_path}")" --s3-no-check-bucket
rclone copyto "${dump_path}" "${GLACIER_ARCHIVE_URI%/}/$(basename "${dump_path}")" --s3-no-check-bucket
echo "cold archive retained for ${COLD_ARCHIVE_RETENTION_YEARS} years: ${dump_path}"
