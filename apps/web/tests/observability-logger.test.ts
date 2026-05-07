import assert from "node:assert/strict";
import { test } from "node:test";

import {
  HARD_DENY_KEYS_FOR_TEST,
  createLogger,
  type LogRecord,
} from "../lib/observability/logger.js";

function captureLogger() {
  const records: LogRecord[] = [];
  const logger = createLogger({
    service: "web",
    version: "0.0.0-test",
    minLevel: "debug",
    sink: (r) => records.push(r),
    now: () => new Date("2026-05-07T00:00:00.000Z"),
  });
  return { logger, records };
}

test("logger emits structured JSON record with frozen timestamp", () => {
  const { logger, records } = captureLogger();
  logger.info("order.submit", "submitting order", { market_id: "mkt_1", duration_ms: 42 });

  assert.equal(records.length, 1);
  const r = records[0]!;
  assert.equal(r.timestamp, "2026-05-07T00:00:00.000Z");
  assert.equal(r.level, "info");
  assert.equal(r.service, "web");
  assert.equal(r.event, "order.submit");
  assert.equal(r.market_id, "mkt_1");
  assert.equal(r.duration_ms, 42);
});

test("logger drops fields outside the SAFE_KEYS allowlist", () => {
  const { logger, records } = captureLogger();
  logger.info("order.submit", "ok", {
    market_id: "mkt_1",
    full_name: "Eric Wang",
    email: "eric@example.com",
    note: "a free-form note we never asked for",
  });

  const r = records[0]!;
  assert.equal(r.market_id, "mkt_1");
  assert.equal(r.full_name, undefined);
  assert.equal(r.email, undefined);
  assert.equal(r.note, undefined);
});

test("logger scrubs PII shapes inside msg as a last-resort defence", () => {
  const { logger, records } = captureLogger();
  logger.info(
    "support.lookup",
    "user eric@example.com phone 0912345678 card 4111111111111111 twid A123456789",
    {},
  );

  const msg = String(records[0]!.msg);
  assert.match(msg, /\[email\]/);
  assert.match(msg, /\[phone\]/);
  assert.match(msg, /\[card\]/);
  assert.match(msg, /\[twid\]/);
  assert.doesNotMatch(msg, /eric@example\.com/);
  assert.doesNotMatch(msg, /0912345678/);
  assert.doesNotMatch(msg, /4111111111111111/);
  assert.doesNotMatch(msg, /A123456789/);
});

test("logger respects minLevel filter", () => {
  const records: LogRecord[] = [];
  const logger = createLogger({
    service: "web",
    version: "0.0.0-test",
    minLevel: "warn",
    sink: (r) => records.push(r),
  });
  logger.debug("x", "debug");
  logger.info("x", "info");
  logger.warn("x", "warn");
  logger.error("x", "error");
  logger.critical("x", "critical");

  assert.equal(records.length, 3);
  assert.deepEqual(
    records.map((r) => r.level),
    ["warn", "error", "critical"],
  );
});

test("logger refuses nested objects and bigints become strings", () => {
  const { logger, records } = captureLogger();
  logger.info("x", "y", {
    market_id: "mkt_1",
    txn_id: 9007199254740993n,
    duration_ms: { nested: true } as unknown as number,
  });
  const r = records[0]!;
  assert.equal(r.txn_id, "9007199254740993");
  assert.equal(r.duration_ms, "[unsupported]");
});

// Exported for test introspection — confirms HARD_DENY_KEYS exists and contains
// expected sensitive keys. We export from logger.ts via the test re-export path.
test("HARD_DENY_KEYS exposes a non-empty set of high-risk fields", () => {
  assert.ok(HARD_DENY_KEYS_FOR_TEST.has("email"));
  assert.ok(HARD_DENY_KEYS_FOR_TEST.has("password"));
  assert.ok(HARD_DENY_KEYS_FOR_TEST.has("authorization"));
});
