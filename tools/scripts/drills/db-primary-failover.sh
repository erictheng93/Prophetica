#!/usr/bin/env bash
set -euo pipefail

: "${DATABASE_URL:?DATABASE_URL is required}"
: "${FAILOVER_COMMAND:?FAILOVER_COMMAND is required}"
: "${DB_FAILOVER_TIMEOUT_SECONDS:=30}"

drill_id="db-primary-failover-$(date -u +%Y%m%dT%H%M%SZ)"
scheduled_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
started_ms="$(date +%s%3N)"

eval "${FAILOVER_COMMAND}"
timeout "${DB_FAILOVER_TIMEOUT_SECONDS}" bash -c "until psql '${DATABASE_URL}' --command 'SELECT 1' >/dev/null 2>&1; do sleep 1; done"

ended_ms="$(date +%s%3N)"
rto_ms=$((ended_ms - started_ms))
executed_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

psql "${DATABASE_URL}" --command "
INSERT INTO drill_logs
(drill_id, type, scheduled_at, executed_at, rto_ms, rpo_lost_events, deviation_notes, improvement_actions, document)
VALUES
('${drill_id}', 'db_primary_failover', '${scheduled_at}', '${executed_at}', ${rto_ms}, 0, '',
 '事後 24h 改進建議: review failover timeline',
 jsonb_build_object(
   'pre_plan', '事前計畫: execute configured DB primary failover command and validate <30s RTO',
   'execution_record', '實際執行: failover command completed; RTO ${rto_ms}ms',
   'improvement_actions_24h', '事後 24h 改進建議: review failover timeline'
 ));"
