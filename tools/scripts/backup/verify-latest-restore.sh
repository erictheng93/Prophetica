#!/usr/bin/env bash
set -euo pipefail

: "${LATEST_BASEBACKUP_ARCHIVE:?LATEST_BASEBACKUP_ARCHIVE is required}"
: "${RESTORE_SANDBOX_PGDATA:=/tmp/prophetica-restore-sandbox}"
: "${RESTORE_VERIFY_DATABASE_URL:?RESTORE_VERIFY_DATABASE_URL is required}"
: "${PRIMARY_DATABASE_URL:?PRIMARY_DATABASE_URL is required}"
: "${METRICS_FILE:=/var/lib/node_exporter/textfile_collector/backup_restore.prom}"
: "${INVALID_MARKER_DIR:=/var/lib/prophetica/backups/invalid}"

tmp="${METRICS_FILE}.tmp"
mkdir -p "$(dirname "${METRICS_FILE}")" "${INVALID_MARKER_DIR}"

rm -rf "${RESTORE_SANDBOX_PGDATA}"
mkdir -p "${RESTORE_SANDBOX_PGDATA}"
zstd -dc "${LATEST_BASEBACKUP_ARCHIVE}" | tar -C "${RESTORE_SANDBOX_PGDATA}" --strip-components=1 -xf -

schema_ok=1
for table in ledger_entries ledger_transactions idempotency_records; do
  if ! psql "${RESTORE_VERIFY_DATABASE_URL}" --tuples-only --no-align --command "SELECT to_regclass('${table}')" | grep -qx "${table}"; then
    schema_ok=0
  fi
done

row_counts_match=1
for table in ledger_entries ledger_transactions; do
  primary_count="$(psql "${PRIMARY_DATABASE_URL}" --tuples-only --no-align --command "SELECT COUNT(*) FROM ${table}")"
  restore_count="$(psql "${RESTORE_VERIFY_DATABASE_URL}" --tuples-only --no-align --command "SELECT COUNT(*) FROM ${table}")"
  if [ "${primary_count}" != "${restore_count}" ]; then
    row_counts_match=0
  fi
done

invalid=0
success=0
if [ "${schema_ok}" -eq 1 ] && [ "${row_counts_match}" -eq 1 ]; then
  success=1
else
  invalid=1
  touch "${INVALID_MARKER_DIR}/$(basename "${LATEST_BASEBACKUP_ARCHIVE}").INVALID"
fi

cat > "${tmp}" <<EOF
backup_restore_success_total{type="weekly"} ${success}
backup_restore_invalid_total{type="weekly"} ${invalid}
EOF
mv "${tmp}" "${METRICS_FILE}"

if [ "${invalid}" -ne 0 ]; then
  exit 2
fi
