import assert from "node:assert/strict";
import { test } from "node:test";

import {
  KYC_METRIC_NAMES,
  KYC_WEBHOOK_WAIT_BUCKETS_SECONDS,
  createKycMetrics,
  emitKycMetric,
  validateKycMetricEvent,
  type KycMetricEvent,
} from "../lib/telemetry/kyc-metrics.js";
import { createPromRegistry } from "../lib/telemetry/prom-registry.js";

test("KYC metric name list matches SPEC-005a §7.3 exactly", () => {
  // Locking the exact set so a future rename or omission shows up as a
  // breakage of the SPEC compatibility contract, not a silent drift.
  assert.deepEqual([...KYC_METRIC_NAMES].sort(), [
    "proph_kyc_session_abandoned_total",
    "proph_kyc_session_completed_total",
    "proph_kyc_session_error_total",
    "proph_kyc_session_started_total",
    "proph_kyc_webhook_wait_seconds",
  ]);
});

test("validateKycMetricEvent enforces enum-bounded labels (PII gate)", () => {
  // tier
  assert.deepEqual(
    validateKycMetricEvent({
      name: "proph_kyc_session_started_total",
      labels: { tier: "TIER_1", provider: "iproov" },
    }),
    { error: "label tier invalid" },
  );
  // provider
  assert.deepEqual(
    validateKycMetricEvent({
      name: "proph_kyc_session_started_total",
      labels: { tier: "TIER_2", provider: "rogue" },
    }),
    { error: "label provider invalid" },
  );
  // outcome
  assert.deepEqual(
    validateKycMetricEvent({
      name: "proph_kyc_session_completed_total",
      labels: { tier: "TIER_2", provider: "iproov", outcome: "ok" },
    }),
    { error: "label outcome invalid" },
  );
  // step_at_abandon
  assert.deepEqual(
    validateKycMetricEvent({
      name: "proph_kyc_session_abandoned_total",
      labels: {
        tier: "TIER_2",
        provider: "iproov",
        step_at_abandon: "scrubbing-pii-here",
      },
    }),
    { error: "label step_at_abandon invalid" },
  );
  // code
  assert.deepEqual(
    validateKycMetricEvent({
      name: "proph_kyc_session_error_total",
      labels: {
        tier: "TIER_2",
        provider: "iproov",
        code: "free text leak",
        recoverable: "true",
      },
    }),
    { error: "label code invalid" },
  );
  // recoverable
  assert.deepEqual(
    validateKycMetricEvent({
      name: "proph_kyc_session_error_total",
      labels: {
        tier: "TIER_2",
        provider: "iproov",
        code: "network_lost",
        recoverable: "yes",
      },
    }),
    { error: "label recoverable must be 'true' or 'false'" },
  );
});

test("validateKycMetricEvent accepts every well-formed payload shape", () => {
  const valid: KycMetricEvent[] = [
    {
      name: "proph_kyc_session_started_total",
      labels: { tier: "TIER_2", provider: "mock" },
    },
    {
      name: "proph_kyc_session_completed_total",
      labels: { tier: "TIER_3", provider: "iproov", outcome: "passed" },
    },
    {
      name: "proph_kyc_session_abandoned_total",
      labels: { tier: "TIER_2", provider: "mock", step_at_abandon: "selfie" },
    },
    {
      name: "proph_kyc_session_error_total",
      labels: {
        tier: "TIER_2",
        provider: "iproov",
        code: "polling_timeout",
        recoverable: "true",
      },
    },
    {
      name: "proph_kyc_webhook_wait_seconds",
      labels: { tier: "TIER_2", outcome: "passed" },
      observation: 42.5,
    },
  ];

  for (const event of valid) {
    const result = validateKycMetricEvent(event);
    assert.ok(!("error" in result), `expected ${event.name} to validate`);
  }
});

test("validateKycMetricEvent rejects negative or non-finite observations", () => {
  for (const observation of [-1, NaN, Infinity, -Infinity]) {
    assert.deepEqual(
      validateKycMetricEvent({
        name: "proph_kyc_webhook_wait_seconds",
        labels: { tier: "TIER_2", outcome: "passed" },
        observation,
      }),
      { error: "observation must be a non-negative finite number" },
    );
  }
});

test("createKycMetrics records counter increments and Histogram observations", () => {
  const metrics = createKycMetrics();

  metrics.record({
    name: "proph_kyc_session_started_total",
    labels: { tier: "TIER_2", provider: "iproov" },
  });
  metrics.record({
    name: "proph_kyc_session_started_total",
    labels: { tier: "TIER_2", provider: "iproov" },
  });
  metrics.record({
    name: "proph_kyc_webhook_wait_seconds",
    labels: { tier: "TIER_2", outcome: "passed" },
    observation: 90,
  });

  const text = metrics.registry.exposition();
  assert.match(
    text,
    /proph_kyc_session_started_total\{tier="TIER_2",provider="iproov"\} 2/,
  );
  assert.match(
    text,
    /proph_kyc_webhook_wait_seconds_count\{tier="TIER_2",outcome="passed"\} 1/,
  );
  assert.match(
    text,
    /proph_kyc_webhook_wait_seconds_sum\{tier="TIER_2",outcome="passed"\} 90/,
  );
});

test("histogram buckets match SPEC-005a section 7.3 defaults", () => {
  assert.deepEqual(
    KYC_WEBHOOK_WAIT_BUCKETS_SECONDS,
    [1, 5, 15, 30, 60, 120, 300],
  );
});

test("emitKycMetric POSTs JSON to /api/metrics/kyc with credentials omitted", async () => {
  const calls: Array<{ url: string; init: RequestInit | undefined }> = [];
  const fakeFetch: typeof fetch = async (input, init) => {
    calls.push({
      url: typeof input === "string" ? input : input.toString(),
      init,
    });
    return new Response(null, { status: 204 });
  };

  await emitKycMetric(
    {
      name: "proph_kyc_session_started_total",
      labels: { tier: "TIER_2", provider: "iproov" },
    },
    { fetcher: fakeFetch },
  );

  assert.equal(calls.length, 1);
  assert.equal(calls[0]?.url, "/api/metrics/kyc");
  assert.equal(calls[0]?.init?.method, "POST");
  assert.equal(calls[0]?.init?.credentials, "omit");
  assert.equal(
    JSON.parse(String(calls[0]?.init?.body)).labels.provider,
    "iproov",
  );
});

test("emitKycMetric never throws even when fetcher rejects", async () => {
  const fakeFetch: typeof fetch = async () => {
    throw new Error("network is down");
  };

  // The promise resolves cleanly — UI must never break because of telemetry.
  await emitKycMetric(
    {
      name: "proph_kyc_session_error_total",
      labels: {
        tier: "TIER_2",
        provider: "iproov",
        code: "network_lost",
        recoverable: "true",
      },
    },
    { fetcher: fakeFetch },
  );
});

test("registry shape mismatch on re-register is loud, not silent", () => {
  const registry = createPromRegistry();
  registry.counter("metric_x", "help", ["a"]);
  assert.throws(
    () => registry.histogram("metric_x", "help", ["a"], [1, 2]),
    /re-registered as histogram/,
  );
  assert.throws(
    () => registry.counter("metric_x", "help", ["a", "b"]),
    /different label set/,
  );
});
