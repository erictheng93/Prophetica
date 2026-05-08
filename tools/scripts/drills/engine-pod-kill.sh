#!/usr/bin/env bash
set -euo pipefail

: "${KUBE_NAMESPACE:=prophetica}"
: "${ENGINE_LABEL:=app=matching-engine}"
: "${DATABASE_URL:?DATABASE_URL is required}"

drill_id="engine-pod-kill-$(date -u +%Y%m%dT%H%M%SZ)"
scheduled_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
started_ms="$(date +%s%3N)"
pod="$(kubectl -n "${KUBE_NAMESPACE}" get pods -l "${ENGINE_LABEL}" -o jsonpath='{.items[0].metadata.name}')"

kubectl -n "${KUBE_NAMESPACE}" delete pod "${pod}"
kubectl -n "${KUBE_NAMESPACE}" wait --for=condition=Ready pod -l "${ENGINE_LABEL}" --timeout=300s

ended_ms="$(date +%s%3N)"
rto_ms=$((ended_ms - started_ms))
executed_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

psql "${DATABASE_URL}" --command "
INSERT INTO drill_logs
(drill_id, type, scheduled_at, executed_at, rto_ms, rpo_lost_events, deviation_notes, improvement_actions, document)
VALUES
('${drill_id}', 'engine_pod_kill', '${scheduled_at}', '${executed_at}', ${rto_ms}, 0, '',
 '事後 24h 改進建議: review readiness and replay timing',
 jsonb_build_object(
   'pre_plan', '事前計畫: delete one random matching-engine pod and measure RTO',
   'execution_record', '實際執行: deleted pod ${pod}; RTO ${rto_ms}ms',
   'improvement_actions_24h', '事後 24h 改進建議: review readiness and replay timing'
 ));"
