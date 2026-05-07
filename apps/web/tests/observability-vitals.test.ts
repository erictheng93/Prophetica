import assert from "node:assert/strict";
import { test } from "node:test";

import {
  installBeacon,
  observeVitals,
  rate,
} from "../lib/observability/vitals.js";

test("rate maps Core Web Vitals values to good / needs-improvement / poor", () => {
  assert.equal(rate("LCP", 2000), "good");
  assert.equal(rate("LCP", 3000), "needs-improvement");
  assert.equal(rate("LCP", 5000), "poor");

  assert.equal(rate("INP", 100), "good");
  assert.equal(rate("INP", 300), "needs-improvement");
  assert.equal(rate("INP", 800), "poor");

  assert.equal(rate("CLS", 0.05), "good");
  assert.equal(rate("CLS", 0.2), "needs-improvement");
  assert.equal(rate("CLS", 0.5), "poor");

  assert.equal(rate("TTFB", 500), "good");
  assert.equal(rate("TTFB", 1500), "needs-improvement");
  assert.equal(rate("TTFB", 2500), "poor");
});

test("observeVitals is a safe no-op when PerformanceObserver is unavailable", () => {
  // node:test runs in Node where PerformanceObserver may exist but our code
  // path is the safeObserve fallback. We just assert no throw and a callable
  // teardown function.
  const stop = observeVitals({ onSample: () => {} });
  assert.equal(typeof stop, "function");
  stop();
});

test("installBeacon is a safe no-op without document/navigator (SSR / Node)", () => {
  const teardown = installBeacon({
    endpoint: "https://example/beacon",
    buildPayload: () => ({ x: 1 }),
  });
  assert.equal(typeof teardown, "function");
  teardown();
});
