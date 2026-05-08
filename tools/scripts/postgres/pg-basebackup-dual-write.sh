#!/usr/bin/env bash
set -euo pipefail

: "${PGHOST:?PGHOST is required}"
: "${PGUSER:?PGUSER is required}"
: "${PGDATABASE:=postgres}"
: "${BACKUP_WORKDIR:=/var/backups/prophetica/postgres}"
: "${R2_BASEBACKUP_URI:?R2_BASEBACKUP_URI is required}"
: "${S3_BASEBACKUP_URI:?S3_BASEBACKUP_URI is required}"
: "${BASEBACKUP_RETENTION_DAYS:=14}"

stamp="$(date -u +%Y%m%dT%H%M%SZ)"
backup_dir="${BACKUP_WORKDIR}/basebackup-${stamp}"
archive_path="${backup_dir}.tar.zst"

mkdir -p "${BACKUP_WORKDIR}"
pg_basebackup \
  --format=tar \
  --gzip \
  --wal-method=stream \
  --checkpoint=fast \
  --label="prophetica-${stamp}" \
  --pgdata="${backup_dir}"

tar -C "${BACKUP_WORKDIR}" -cf - "$(basename "${backup_dir}")" | zstd -T0 -o "${archive_path}"
rclone copyto "${archive_path}" "${R2_BASEBACKUP_URI%/}/$(basename "${archive_path}")" --s3-no-check-bucket
rclone copyto "${archive_path}" "${S3_BASEBACKUP_URI%/}/$(basename "${archive_path}")" --s3-no-check-bucket

find "${BACKUP_WORKDIR}" -maxdepth 1 -type f -name 'basebackup-*.tar.zst' -mtime +"${BASEBACKUP_RETENTION_DAYS}" -delete
find "${BACKUP_WORKDIR}" -maxdepth 1 -type d -name 'basebackup-*' -mtime +"${BASEBACKUP_RETENTION_DAYS}" -exec rm -rf {} +
