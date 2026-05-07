#!/usr/bin/env bash
set -euo pipefail

: "${TOKYO_KAFKA_BOOTSTRAP_SERVERS:?TOKYO_KAFKA_BOOTSTRAP_SERVERS is required}"
: "${SINGAPORE_KAFKA_BOOTSTRAP_SERVERS:?SINGAPORE_KAFKA_BOOTSTRAP_SERVERS is required}"
: "${MM2_TOPICS_REGEX:=engine.events.*|ledger.txns.*|settlement.*}"
: "${METRICS_FILE:=/var/lib/node_exporter/textfile_collector/kafka_mm2_lag.prom}"

tmp="${METRICS_FILE}.tmp"
mkdir -p "$(dirname "${METRICS_FILE}")"
: > "${tmp}"

kafka-topics --bootstrap-server "${SINGAPORE_KAFKA_BOOTSTRAP_SERVERS}" --list \
  | grep -E "${MM2_TOPICS_REGEX}" \
  | while read -r topic; do
      singapore_latest="$(kafka-run-class kafka.tools.GetOffsetShell --broker-list "${SINGAPORE_KAFKA_BOOTSTRAP_SERVERS}" --topic "${topic}" --time -1 | awk -F: '{sum += $3} END {print sum + 0}')"
      tokyo_latest="$(kafka-run-class kafka.tools.GetOffsetShell --broker-list "${TOKYO_KAFKA_BOOTSTRAP_SERVERS}" --topic "${topic}" --time -1 | awk -F: '{sum += $3} END {print sum + 0}')"
      lag_offsets=$((tokyo_latest - singapore_latest))
      if [ "${lag_offsets}" -lt 0 ]; then
        lag_offsets=0
      fi
      # Conservative conversion until per-topic event-rate metrics are wired.
      lag_seconds="${lag_offsets}"
      printf 'kafka_mm2_lag_seconds{topic="%s"} %s\n' "${topic}" "${lag_seconds}" >> "${tmp}"
    done

mv "${tmp}" "${METRICS_FILE}"
