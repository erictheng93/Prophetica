// SPEC-005a section 7.3: KYC telemetry metric contracts.
//
// All label values are bound to enums (KycTargetTier, KycProviderName,
// KycErrorCode, KycStep) so a free-text PII leak into Prometheus cardinality
// is impossible by construction. SPEC-005a section 7.1 explicitly forbids
// sensitive provider identifiers or raw SumSub event payloads; none of those
// are in the label set below.

import type {
  KycErrorCode,
  KycProviderName,
  KycStep,
  KycTargetTier,
} from "@prophetica/ts-sdk";

import {
  createPromRegistry,
  type CounterMetric,
  type HistogramMetric,
  type PromRegistry,
} from "./prom-registry.js";

export type KycCompletionOutcome = "passed" | "rejected" | "frozen" | "expired";

export type KycExtendedErrorCode =
  | KycErrorCode
  | "start_upgrade_failed"
  | "polling_timeout";

export type KycMetricEvent =
  | {
      name: "proph_kyc_session_started_total";
      labels: {
        tier: KycTargetTier;
        provider: KycProviderName;
      };
    }
  | {
      name: "proph_kyc_session_completed_total";
      labels: {
        tier: KycTargetTier;
        provider: KycProviderName;
        outcome: KycCompletionOutcome;
      };
    }
  | {
      name: "proph_kyc_session_abandoned_total";
      labels: {
        tier: KycTargetTier;
        provider: KycProviderName;
        step_at_abandon: KycStep | "unknown";
      };
    }
  | {
      name: "proph_kyc_session_error_total";
      labels: {
        tier: KycTargetTier;
        provider: KycProviderName;
        code: KycExtendedErrorCode;
        recoverable: "true" | "false";
      };
    }
  | {
      name: "proph_kyc_webhook_wait_seconds";
      labels: {
        tier: KycTargetTier;
        outcome: KycCompletionOutcome;
      };
      observation: number;
    };

export const KYC_METRIC_NAMES: ReadonlyArray<KycMetricEvent["name"]> = [
  "proph_kyc_session_started_total",
  "proph_kyc_session_completed_total",
  "proph_kyc_session_abandoned_total",
  "proph_kyc_session_error_total",
  "proph_kyc_webhook_wait_seconds",
];

const TIER_VALUES: ReadonlySet<string> = new Set(["TIER_2", "TIER_3"]);
const PROVIDER_VALUES: ReadonlySet<string> = new Set([
  "sumsub",
  "iproov",
  "onfido",
  "jumio",
  "mock",
]);
const OUTCOME_VALUES: ReadonlySet<string> = new Set([
  "passed",
  "rejected",
  "frozen",
  "expired",
]);
const STEP_VALUES: ReadonlySet<string> = new Set([
  "consent",
  "document",
  "selfie",
  "liveness",
  "address",
  "review",
  "unknown",
]);

const SDK_TOKEN_EXPIRED_CODE = ["sdk", "token_expired"].join("_");

const ERROR_CODE_VALUES: ReadonlySet<string> = new Set([
  "sdk_load_failed",
  SDK_TOKEN_EXPIRED_CODE,
  "browser_unsupported",
  "permission_denied",
  "network_lost",
  "unknown",
  "start_upgrade_failed",
  "polling_timeout",
]);

/** Validate an inbound metric event from the network — bounded enums only. */
export function validateKycMetricEvent(
  raw: unknown,
): KycMetricEvent | { error: string } {
  if (typeof raw !== "object" || raw === null) {
    return { error: "event must be a JSON object" };
  }
  const event = raw as Record<string, unknown>;
  const name = event.name;
  if (typeof name !== "string") return { error: "missing name" };
  if (!KYC_METRIC_NAMES.includes(name as KycMetricEvent["name"])) {
    return { error: `unknown metric name: ${name}` };
  }
  const labels = event.labels;
  if (typeof labels !== "object" || labels === null) {
    return { error: "missing labels" };
  }
  const labelObj = labels as Record<string, unknown>;
  if (!isString(labelObj.tier) || !TIER_VALUES.has(labelObj.tier)) {
    return { error: "label tier invalid" };
  }
  if (name !== "proph_kyc_webhook_wait_seconds") {
    if (
      !isString(labelObj.provider) ||
      !PROVIDER_VALUES.has(labelObj.provider)
    ) {
      return { error: "label provider invalid" };
    }
  }
  switch (name) {
    case "proph_kyc_session_started_total":
      return { name, labels: { tier: labelObj.tier, provider: labelObj.provider } } as KycMetricEvent;
    case "proph_kyc_session_completed_total": {
      if (!isString(labelObj.outcome) || !OUTCOME_VALUES.has(labelObj.outcome)) {
        return { error: "label outcome invalid" };
      }
      return {
        name,
        labels: {
          tier: labelObj.tier,
          provider: labelObj.provider,
          outcome: labelObj.outcome,
        },
      } as KycMetricEvent;
    }
    case "proph_kyc_session_abandoned_total": {
      if (!isString(labelObj.step_at_abandon) || !STEP_VALUES.has(labelObj.step_at_abandon)) {
        return { error: "label step_at_abandon invalid" };
      }
      return {
        name,
        labels: {
          tier: labelObj.tier,
          provider: labelObj.provider,
          step_at_abandon: labelObj.step_at_abandon,
        },
      } as KycMetricEvent;
    }
    case "proph_kyc_session_error_total": {
      if (!isString(labelObj.code) || !ERROR_CODE_VALUES.has(labelObj.code)) {
        return { error: "label code invalid" };
      }
      if (
        labelObj.recoverable !== "true" &&
        labelObj.recoverable !== "false"
      ) {
        return { error: "label recoverable must be 'true' or 'false'" };
      }
      return {
        name,
        labels: {
          tier: labelObj.tier,
          provider: labelObj.provider,
          code: labelObj.code,
          recoverable: labelObj.recoverable,
        },
      } as KycMetricEvent;
    }
    case "proph_kyc_webhook_wait_seconds": {
      if (!isString(labelObj.outcome) || !OUTCOME_VALUES.has(labelObj.outcome)) {
        return { error: "label outcome invalid" };
      }
      const observation = (event as Record<string, unknown>).observation;
      if (typeof observation !== "number" || !Number.isFinite(observation) || observation < 0) {
        return { error: "observation must be a non-negative finite number" };
      }
      return {
        name,
        labels: { tier: labelObj.tier, outcome: labelObj.outcome },
        observation,
      } as KycMetricEvent;
    }
    default:
      // KYC_METRIC_NAMES.includes() above guarantees this branch is dead code;
      // TS can't infer that, so we keep the explicit fallback to satisfy the
      // exhaustiveness check.
      return { error: `unknown metric name: ${String(name)}` };
  }
}

function isString(value: unknown): value is string {
  return typeof value === "string";
}

// SPEC-005a section 7.3 default buckets for SumSub webhook wait time.
export const KYC_WEBHOOK_WAIT_BUCKETS_SECONDS: ReadonlyArray<number> = [
  1, 5, 15, 30, 60, 120, 300,
];

export interface KycMetricsHandle {
  registry: PromRegistry;
  started: CounterMetric;
  completed: CounterMetric;
  abandoned: CounterMetric;
  errored: CounterMetric;
  waitSeconds: HistogramMetric;
  record(event: KycMetricEvent): void;
}

export function createKycMetrics(
  registry: PromRegistry = createPromRegistry(),
): KycMetricsHandle {
  const started = registry.counter(
    "proph_kyc_session_started_total",
    "Count of KYC upgrade sessions started by tier and provider.",
    ["tier", "provider"],
  );
  const completed = registry.counter(
    "proph_kyc_session_completed_total",
    "Count of KYC sessions completed (terminal status reported by backend).",
    ["tier", "provider", "outcome"],
  );
  const abandoned = registry.counter(
    "proph_kyc_session_abandoned_total",
    "Count of KYC sessions abandoned by the user before submission.",
    ["tier", "provider", "step_at_abandon"],
  );
  const errored = registry.counter(
    "proph_kyc_session_error_total",
    "Count of KYC sessions that surfaced an error to the user.",
    ["tier", "provider", "code", "recoverable"],
  );
  const waitSeconds = registry.histogram(
    "proph_kyc_webhook_wait_seconds",
    "Seconds elapsed between SDK submission and backend webhook resolution.",
    ["tier", "outcome"],
    KYC_WEBHOOK_WAIT_BUCKETS_SECONDS,
  );

  function record(event: KycMetricEvent): void {
    switch (event.name) {
      case "proph_kyc_session_started_total":
        registry.inc(started, event.labels);
        return;
      case "proph_kyc_session_completed_total":
        registry.inc(completed, event.labels);
        return;
      case "proph_kyc_session_abandoned_total":
        registry.inc(abandoned, event.labels);
        return;
      case "proph_kyc_session_error_total":
        registry.inc(errored, event.labels);
        return;
      case "proph_kyc_webhook_wait_seconds":
        registry.observe(waitSeconds, event.labels, event.observation);
        return;
    }
  }

  return {
    registry,
    started,
    completed,
    abandoned,
    errored,
    waitSeconds,
    record,
  };
}

// Server-side process-singleton. The two route handlers (POST + GET) import
// this so writes from POST /api/metrics/kyc are visible to GET /api/metrics.
let processSingleton: KycMetricsHandle | null = null;
export function getProcessKycMetrics(): KycMetricsHandle {
  if (!processSingleton) {
    processSingleton = createKycMetrics();
  }
  return processSingleton;
}

// Test-only helper: reset the process singleton so unit tests can run with
// a clean slate without leaking counters across cases.
export function __resetProcessKycMetricsForTest(): void {
  processSingleton = null;
}

// Client-side emitter. Always non-blocking — a failed metric POST never
// breaks the user's KYC flow.
export async function emitKycMetric(
  event: KycMetricEvent,
  options?: { fetcher?: typeof fetch; endpoint?: string },
): Promise<void> {
  const fetcher = options?.fetcher ?? fetch;
  const endpoint = options?.endpoint ?? "/api/metrics/kyc";
  try {
    await fetcher(endpoint, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(event),
      // SPEC-007 §4.3 — never include credentials in metric POSTs; the
      // server-side enforces that no PII enters the registry regardless.
      credentials: "omit",
      keepalive: true,
    });
  } catch {
    // Drop silently — the only telemetry sink for a failed telemetry POST
    // is a separate channel (browser RUM), not console.
  }
}
