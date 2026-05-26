import assert from "node:assert/strict";
import { test } from "node:test";

import {
  handleKycMetricsGet,
  handleKycMetricsPost,
} from "../lib/telemetry/kyc-metrics-handler.js";
import {
  __resetProcessKycMetricsForTest,
  getProcessKycMetrics,
} from "../lib/telemetry/kyc-metrics.js";

function jsonBody(body: unknown): Request {
  return new Request("http://localhost/api/metrics/kyc", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
}

test("POST /api/metrics/kyc accepts a valid event and returns 204", async () => {
  __resetProcessKycMetricsForTest();
  const result = await handleKycMetricsPost(
    jsonBody({
      name: "proph_kyc_session_started_total",
      labels: { tier: "TIER_2", provider: "iproov" },
    }),
  );
  assert.equal(result.status, 204);
});

test("POST /api/metrics/kyc rejects invalid JSON with 400", async () => {
  __resetProcessKycMetricsForTest();
  const result = await handleKycMetricsPost(
    new Request("http://localhost/api/metrics/kyc", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: "{not-json",
    }),
  );
  assert.equal(result.status, 400);
  if (result.status === 400) {
    assert.equal(result.body.error, "invalid_json");
  }
});

test("POST /api/metrics/kyc rejects an unknown metric name with 400", async () => {
  __resetProcessKycMetricsForTest();
  const result = await handleKycMetricsPost(
    jsonBody({
      name: "rogue_metric",
      labels: { tier: "TIER_2", provider: "iproov" },
    }),
  );
  assert.equal(result.status, 400);
});

test("POST then GET produces a Prom exposition with the recorded counter", async () => {
  __resetProcessKycMetricsForTest();
  await handleKycMetricsPost(
    jsonBody({
      name: "proph_kyc_session_started_total",
      labels: { tier: "TIER_2", provider: "iproov" },
    }),
  );
  await handleKycMetricsPost(
    jsonBody({
      name: "proph_kyc_session_started_total",
      labels: { tier: "TIER_2", provider: "iproov" },
    }),
  );

  const result = handleKycMetricsGet();

  assert.equal(result.status, 200);
  assert.equal(
    result.headers["content-type"],
    "application/openmetrics-text; version=1.0.0; charset=utf-8",
  );
  assert.match(result.body, /^# HELP proph_kyc_session_started_total /m);
  assert.match(
    result.body,
    /^# TYPE proph_kyc_session_started_total counter$/m,
  );
  assert.match(
    result.body,
    /proph_kyc_session_started_total\{tier="TIER_2",provider="iproov"\} 2/,
  );
  assert.match(result.body, /^# EOF$/m);
});

test("getProcessKycMetrics returns the same singleton across calls", () => {
  __resetProcessKycMetricsForTest();
  const a = getProcessKycMetrics();
  const b = getProcessKycMetrics();
  assert.strictEqual(a, b);
});
