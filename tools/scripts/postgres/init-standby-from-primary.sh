#!/usr/bin/env bash
set -euo pipefail

: "${PRIMARY_HOST:=tokyo-primary}"
: "${PRIMARY_PORT:=5432}"
: "${REPLICATION_USER:=replicator}"
: "${PGDATA:?PGDATA is required}"

if [ -s "${PGDATA}/PG_VERSION" ]; then
  echo "standby data directory already initialized: ${PGDATA}"
  exit 0
fi

rm -rf "${PGDATA:?}/"*
pg_basebackup \
  --host="${PRIMARY_HOST}" \
  --port="${PRIMARY_PORT}" \
  --username="${REPLICATION_USER}" \
  --pgdata="${PGDATA}" \
  --wal-method=stream \
  --write-recovery-conf \
  --checkpoint=fast

cat >> "${PGDATA}/postgresql.auto.conf" <<'EOF'
primary_slot_name = 'standby1'
primary_conninfo = 'host=tokyo-primary port=5432 user=replicator application_name=standby1 passfile=/var/lib/postgresql/.pgpass'
EOF
