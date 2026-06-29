import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const script = await readFile(
  new URL("./matching-engine-compose-smoke.sh", import.meta.url),
  "utf8",
);

test("compose smoke verifies Kafka projection delivery", () => {
  assert.match(script, /KAFKA_PROJECTION_TOPIC/);
  assert.match(script, /engine\.order_update/);
  assert.match(script, /kafka-console-consumer/);
  assert.match(script, /kind.+order_update/s);
});

test("compose smoke keeps HTTP readiness and order book checks", () => {
  assert.match(script, /\/healthz\/ready/);
  assert.match(script, /\/api\/v1\/markets\/\$\{MARKET_ID\}\/orders/);
  assert.match(script, /\/api\/v1\/markets\/\$\{MARKET_ID\}\/book/);
});

test("compose smoke has a debuggable engine startup timeout", () => {
  assert.match(script, /MATCHING_ENGINE_SMOKE_STARTUP_ATTEMPTS/);
  assert.match(script, /Matching engine did not become ready/);
  assert.match(script, /prophetica-matching-engine-smoke\.log/);
});
