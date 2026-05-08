#!/usr/bin/env bash
set -euo pipefail

wal_path="${1:?wal path required}"
wal_file="${2:?wal file name required}"

: "${R2_WAL_ARCHIVE_URI:?R2_WAL_ARCHIVE_URI is required, e.g. r2:prophetica-wal/tokyo}"
: "${POSTGRES_ARCHIVE_RETRIES:=5}"
: "${POSTGRES_ARCHIVE_RETRY_SECONDS:=5}"

target="${R2_WAL_ARCHIVE_URI%/}/${wal_file}"

attempt=1
while true; do
  if rclone copyto "${wal_path}" "${target}" --s3-no-check-bucket; then
    exit 0
  fi

  if [ "${attempt}" -ge "${POSTGRES_ARCHIVE_RETRIES}" ]; then
    echo "engine.postgres_wal_archive_failed wal_file=${wal_file}" >&2
    exit 1
  fi

  attempt=$((attempt + 1))
  sleep "${POSTGRES_ARCHIVE_RETRY_SECONDS}"
done
