import assert from "node:assert/strict";
import test from "node:test";

import {
  __resetProcessKycMetricsForTest,
  getProcessKycMetrics,
} from "../lib/telemetry/kyc-metrics.js";
import {
  __resetJourneyMetricsForTest,
  recordJourneyRedirect,
} from "../lib/telemetry/journey-metrics.js";

test("journey redirect metrics are exposed through the process metrics registry", () => {
  __resetProcessKycMetricsForTest();
  __resetJourneyMetricsForTest();

  recordJourneyRedirect("/markets/e2e-market?side=yes", "/auth/sign-in");

  const body = getProcessKycMetrics().registry.exposition();
  assert.match(body, /# TYPE journey_redirect_total counter/);
  assert.match(
    body,
    /journey_redirect_total\{from="\/markets\/e2e-market",to="\/auth\/sign-in"\} 1/,
  );
});

test("journey redirect metrics do not expose external destinations as labels", () => {
  __resetProcessKycMetricsForTest();
  __resetJourneyMetricsForTest();

  recordJourneyRedirect("/auth/sign-in", "https://evil.example/phish");

  const body = getProcessKycMetrics().registry.exposition();
  assert.match(
    body,
    /journey_redirect_total\{from="\/auth\/sign-in",to="external"\} 1/,
  );
});
