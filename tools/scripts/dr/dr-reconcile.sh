#!/usr/bin/env bash
set -euo pipefail

: "${PRIMARY_DATABASE_URL:?PRIMARY_DATABASE_URL is required}"
: "${DR_DATABASE_URL:?DR_DATABASE_URL is required}"
: "${TOKYO_KAFKA_BOOTSTRAP_SERVERS:?TOKYO_KAFKA_BOOTSTRAP_SERVERS is required}"
: "${SINGAPORE_KAFKA_BOOTSTRAP_SERVERS:?SINGAPORE_KAFKA_BOOTSTRAP_SERVERS is required}"
: "${METRICS_FILE:=/var/lib/node_exporter/textfile_collector/dr_reconciliation.prom}"
: "${DR_KAFKA_TOPICS_REGEX:=engine.events.*|ledger.txns.*|settlement.*}"

tmp="${METRICS_FILE}.tmp"
mkdir -p "$(dirname "${METRICS_FILE}")"

primary_count="$(psql "${PRIMARY_DATABASE_URL}" --tuples-only --no-align --command 'SELECT COUNT(*) FROM ledger_entries')"
dr_count="$(psql "${DR_DATABASE_URL}" --tuples-only --no-align --command 'SELECT COUNT(*) FROM ledger_entries')"
mismatch=0

if [ "${primary_count}" != "${dr_count}" ]; then
  mismatch=1
fi

max_offset_lag=0
while read -r topic; do
  singapore_latest="$(kafka-run-class kafka.tools.GetOffsetShell --broker-list "${SINGAPORE_KAFKA_BOOTSTRAP_SERVERS}" --topic "${topic}" --time -1 | awk -F: '{sum += $3} END {print sum + 0}')"
  tokyo_latest="$(kafka-run-class kafka.tools.GetOffsetShell --broker-list "${TOKYO_KAFKA_BOOTSTRAP_SERVERS}" --topic "${topic}" --time -1 | awk -F: '{sum += $3} END {print sum + 0}')"
  lag=$((tokyo_latest - singapore_latest))
  if [ "${lag}" -gt "${max_offset_lag}" ]; then
    max_offset_lag="${lag}"
  fi
done < <(kafka-topics --bootstrap-server "${SINGAPORE_KAFKA_BOOTSTRAP_SERVERS}" --list | grep -E "${DR_KAFKA_TOPICS_REGEX}" || true)

if [ "${max_offset_lag}" -gt 1 ]; then
  mismatch=1
fi

cat > "${tmp}" <<EOF
dr_pg_ledger_entries_primary_count ${primary_count}
dr_pg_ledger_entries_standby_count ${dr_count}
dr_kafka_offset_lag_seconds ${max_offset_lag}
dr_reconciliation_mismatch_total ${mismatch}
EOF
mv "${tmp}" "${METRICS_FILE}"

if [ "${mismatch}" -ne 0 ]; then
  exit 2
fi
