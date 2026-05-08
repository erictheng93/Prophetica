// SPEC-005a §7.3 — GET endpoint exposing the in-process Prometheus
// registry as standard text exposition format. Scraped by the platform
// metrics agent (Prometheus, Grafana Agent, OpenTelemetry collector etc.)
// at the deployment level.
//
// Runtime: nodejs to share the module-level singleton with the POST route.

import { handleKycMetricsGet } from "@/lib/telemetry/kyc-metrics-handler";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export function GET(): Response {
  const result = handleKycMetricsGet();
  return new Response(result.body, {
    status: result.status,
    headers: result.headers,
  });
}
