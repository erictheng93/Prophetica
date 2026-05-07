#!/usr/bin/env bash
set -euo pipefail

: "${RESTORE_TARGET_TIME:?RESTORE_TARGET_TIME is required, e.g. 2026-05-07 10:30:00+00}"
: "${BASEBACKUP_ARCHIVE:?BASEBACKUP_ARCHIVE is required}"
: "${RESTORE_PGDATA:?RESTORE_PGDATA is required}"
: "${R2_WAL_ARCHIVE_URI:?R2_WAL_ARCHIVE_URI is required}"

rm -rf "${RESTORE_PGDATA}"
mkdir -p "${RESTORE_PGDATA}"
zstd -dc "${BASEBACKUP_ARCHIVE}" | tar -C "${RESTORE_PGDATA}" --strip-components=1 -xf -

cat > "${RESTORE_PGDATA}/postgresql.auto.conf" <<EOF
restore_command = 'rclone copyto ${R2_WAL_ARCHIVE_URI%/}/%f %p --s3-no-check-bucket'
recovery_target_time = '${RESTORE_TARGET_TIME}'
recovery_target_action = 'pause'
EOF

touch "${RESTORE_PGDATA}/recovery.signal"
echo "PITR restore prepared at ${RESTORE_PGDATA}; start postgres to replay to ${RESTORE_TARGET_TIME}."
