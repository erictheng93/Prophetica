import assert from "node:assert/strict";
import { test } from "node:test";

import {
  budgetMs,
  classifyBudget,
  startMeasure,
  withMeasure,
  type Clock,
} from "../lib/observability/latency.js";

test("classifyBudget matches SPEC-007 §6.4.4 thresholds (budget / 1.5x / 2x)", () => {
  // client.receive budget = 10ms
  assert.equal(classifyBudget("client.receive", 5), "ok");
  assert.equal(classifyBudget("client.receive", 10), "ok");
  assert.equal(classifyBudget("client.receive", 12), "warn");
  assert.equal(classifyBudget("client.receive", 15), "warn");
  assert.equal(classifyBudget("client.receive", 18), "breach");
  assert.equal(classifyBudget("client.receive", 20), "breach");
  assert.equal(classifyBudget("client.receive", 30), "p0");
});

test("budgetMs exposes per-hop budget for log enrichment", () => {
  assert.equal(budgetMs("client.submit"), 5);
  assert.equal(budgetMs("client.receive"), 10);
  assert.equal(budgetMs("client.render"), 16);
});

test("startMeasure produces a Measurement with correct hop and budget", () => {
  let t = 100;
  const clock: Clock = { now: () => t };
  const stop = startMeasure("client.submit", clock);
  t = 103;
  const m = stop();
  assert.equal(m.hop, "client.submit");
  assert.equal(m.budgetMs, 5);
  assert.equal(m.durationMs, 3);
  assert.equal(m.status, "ok");
});

test("withMeasure invokes onComplete once with classified measurement", async () => {
  let t = 0;
  const clock: Clock = { now: () => t };
  type Captured = { durationMs: number; status: string };
  const captures: Captured[] = [];
  const result = await withMeasure(
    "client.receive",
    async () => {
      t = 25;
      return "value";
    },
    {
      clock,
      onComplete: (m) => {
        captures.push({ durationMs: m.durationMs, status: m.status });
      },
    },
  );
  assert.equal(result, "value");
  assert.equal(captures.length, 1);
  const captured = captures[0];
  assert.ok(captured);
  assert.equal(captured.durationMs, 25);
  assert.equal(captured.status, "p0");
});
