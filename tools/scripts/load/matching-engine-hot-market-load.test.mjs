import assert from "node:assert/strict";
import test from "node:test";

import {
  chooseMarket,
  percentile,
  renderMarkdownReport,
  runLoadTest,
  summarizeOperation,
} from "./matching-engine-hot-market-load.mjs";

test("percentile uses the nearest-rank value", () => {
  assert.equal(percentile([10, 20, 30, 40], 50), 20);
  assert.equal(percentile([10, 20, 30, 40], 99), 40);
  assert.equal(percentile([], 99), 0);
});

test("chooseMarket sends the configured skew to the hot market", () => {
  const markets = Array.from({ length: 100 }, (_, index) =>
    chooseMarket(index, "hot", 9, 0.8),
  );

  assert.equal(markets.filter((market) => market === "hot").length, 80);
  assert(markets.some((market) => market.startsWith("mkt-cold-")));
});

test("summarizeOperation reports latency percentiles and error rate", () => {
  const summary = summarizeOperation([
    { status: 200, durationMs: 5 },
    { status: 409, durationMs: 10 },
    { status: 503, durationMs: 50 },
  ]);

  assert.equal(summary.count, 3);
  assert.equal(summary.p50Ms, 10);
  assert.equal(summary.p99Ms, 50);
  assert.equal(summary.errorRate, 2 / 3);
});

test("renderMarkdownReport includes decision evidence", () => {
  const markdown = renderMarkdownReport({
    targetLevel: "L1",
    scenario: "hot-market skew",
    result: "FAIL",
    testDate: "2026-06-29T00:00:00.000Z",
    targetUrl: "https://engine.example",
    workload: {
      hotMarketId: "hot",
      durationSeconds: 60,
      orderTps: 10,
      readQps: 20,
      coldMarketCount: 9,
      hotMarketRatio: 0.8,
    },
    operations: {
      place_order: {
        count: 1,
        p50Ms: 1,
        p95Ms: 2,
        p99Ms: 3,
        p999Ms: 4,
        errorRate: 0,
      },
      get_book: {
        count: 1,
        p50Ms: 1,
        p95Ms: 2,
        p99Ms: 3,
        p999Ms: 4,
        errorRate: 0,
      },
    },
    reason: "order_p99_ms=3.00",
  });

  assert(markdown.includes("Decision: FAIL"));
  assert(markdown.includes("order_p99_ms=3.00"));
});

test("runLoadTest fails when read P99 exceeds its budget", async () => {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = async (url) => {
    await new Promise((resolve) => {
      setTimeout(resolve, url.includes("/book?") ? 3 : 1);
    });
    return {
      status: 200,
      async arrayBuffer() {
        return new ArrayBuffer(0);
      },
    };
  };

  try {
    const report = await runLoadTest({
      targetUrl: "http://127.0.0.1:8088",
      durationSeconds: 0.01,
      orderTps: 1,
      readQps: 1,
      readP99BudgetMs: 1,
    });

    assert.equal(report.result, "FAIL");
    assert.match(report.reason, /read_p99_ms=/);
  } finally {
    globalThis.fetch = originalFetch;
  }
});
