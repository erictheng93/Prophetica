#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
COMPOSE_FILE="${COMPOSE_FILE:-${ROOT_DIR}/infra/docker-compose.local.yaml}"
HTTP_PORT="${MATCHING_ENGINE_SMOKE_HTTP_PORT:-18088}"
KAFKA_BROKERS="${MATCHING_ENGINE_KAFKA_BROKERS:-127.0.0.1:9092}"
KAFKA_PROJECTION_TOPIC="${MATCHING_ENGINE_SMOKE_KAFKA_TOPIC:-engine.order_update}"
MARKET_ID="${MATCHING_ENGINE_SMOKE_MARKET_ID:-mkt-compose-smoke-$(date +%s)}"
STARTUP_ATTEMPTS="${MATCHING_ENGINE_SMOKE_STARTUP_ATTEMPTS:-180}"
ENGINE_LOG="/tmp/prophetica-matching-engine-smoke.log"

cleanup() {
  if [[ -n "${ENGINE_PID:-}" ]]; then
    kill "${ENGINE_PID}" >/dev/null 2>&1 || true
    wait "${ENGINE_PID}" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

cd "${ROOT_DIR}"

docker compose -f "${COMPOSE_FILE}" up -d kafka

for _ in $(seq 1 60); do
  if docker compose -f "${COMPOSE_FILE}" exec -T kafka \
    kafka-broker-api-versions --bootstrap-server localhost:9092 >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

if ! docker compose -f "${COMPOSE_FILE}" exec -T kafka \
  kafka-broker-api-versions --bootstrap-server localhost:9092 >/dev/null 2>&1; then
  echo "Kafka did not become ready" >&2
  exit 1
fi

for topic in engine.order_update engine.trade engine.book_delta; do
  docker compose -f "${COMPOSE_FILE}" exec -T kafka \
    kafka-topics --bootstrap-server localhost:9092 \
    --create --if-not-exists --topic "${topic}" --partitions 3 --replication-factor 1 \
    >/dev/null
done

MATCHING_ENGINE_PORT="${HTTP_PORT}" \
MATCHING_ENGINE_KAFKA_BROKERS="${KAFKA_BROKERS}" \
cargo run -p matching-engine-hot --features kafka --locked >"${ENGINE_LOG}" 2>&1 &
ENGINE_PID=$!

engine_ready=0
for _ in $(seq 1 "${STARTUP_ATTEMPTS}"); do
  if curl -fsS "http://127.0.0.1:${HTTP_PORT}/healthz/ready" >/tmp/prophetica-matching-engine-ready.json; then
    engine_ready=1
    break
  fi
  if ! kill -0 "${ENGINE_PID}" >/dev/null 2>&1; then
    cat "${ENGINE_LOG}" >&2 || true
    exit 1
  fi
  sleep 1
done

if [[ "${engine_ready}" != "1" ]]; then
  echo "Matching engine did not become ready after ${STARTUP_ATTEMPTS}s" >&2
  cat "${ENGINE_LOG}" >&2 || true
  exit 1
fi

curl -fsS "http://127.0.0.1:${HTTP_PORT}/healthz/ready" | tee /tmp/prophetica-matching-engine-ready.json

curl -fsS \
  -H 'content-type: application/json' \
  -H 'x-user-id: 1001' \
  -d '{"side":"sell","outcome":"YES","type":"LIMIT","price_pts":55,"quantity":3,"time_in_force":"GTC"}' \
  "http://127.0.0.1:${HTTP_PORT}/api/v1/markets/${MARKET_ID}/orders" \
  | tee /tmp/prophetica-matching-engine-order.json

curl -fsS "http://127.0.0.1:${HTTP_PORT}/api/v1/markets/${MARKET_ID}/book?depth=5" \
  | tee /tmp/prophetica-matching-engine-book.json

docker compose -f "${COMPOSE_FILE}" exec -T kafka \
  kafka-console-consumer --bootstrap-server localhost:9092 \
  --topic "${KAFKA_PROJECTION_TOPIC}" --from-beginning --max-messages 20 --timeout-ms 10000 \
  >/tmp/prophetica-matching-engine-kafka-events.jsonl 2>/tmp/prophetica-matching-engine-kafka-consumer.log || true

if ! grep -F "\"market_id\":\"${MARKET_ID}\"" /tmp/prophetica-matching-engine-kafka-events.jsonl \
  | grep -F '"kind":"order_update"' >/dev/null; then
  echo "Kafka projection event was not observed on ${KAFKA_PROJECTION_TOPIC}" >&2
  echo "consumer stderr:" >&2
  cat /tmp/prophetica-matching-engine-kafka-consumer.log >&2 || true
  echo "events:" >&2
  cat /tmp/prophetica-matching-engine-kafka-events.jsonl >&2 || true
  exit 1
fi

echo
echo "matching-engine compose smoke passed"
