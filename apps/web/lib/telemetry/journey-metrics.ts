import { getProcessKycMetrics, type KycMetricsHandle } from "./kyc-metrics.js";
import type { CounterMetric } from "./prom-registry.js";

let redirectCounter: CounterMetric | null = null;

export function recordJourneyRedirect(from: string, to: string): void {
  const metrics = getProcessKycMetrics();
  metrics.registry.inc(journeyRedirectCounter(metrics), {
    from: journeyMetricPath(from),
    to: journeyMetricPath(to),
  });
}

function journeyRedirectCounter(metrics: KycMetricsHandle): CounterMetric {
  redirectCounter ??= metrics.registry.counter(
    "journey_redirect_total",
    "Server-side onboarding journey redirects by source and destination path.",
    ["from", "to"],
  );
  return redirectCounter;
}

function journeyMetricPath(value: string): string {
  try {
    const url = new URL(value, "https://prophetica.local");
    if (url.origin !== "https://prophetica.local") {
      return "external";
    }
    const pathname = url.pathname || "/";
    return pathname.length > 120 ? pathname.slice(0, 120) : pathname;
  } catch {
    return "invalid";
  }
}

export function __resetJourneyMetricsForTest(): void {
  redirectCounter = null;
}
