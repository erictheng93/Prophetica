// Pure (Next-free) handler logic for /api/metrics/kyc + /api/metrics. Lives
// in lib/ so unit tests can compile under tsconfig.test.json without pulling
// next/server into the node:test runner. The route files are thin wrappers
// that map this contract to NextResponse.

import {
  getProcessKycMetrics,
  validateKycMetricEvent,
} from "./kyc-metrics.js";

export interface MetricsPostOk {
  status: 204;
}

export interface MetricsPostErr {
  status: 400;
  body: { error: string };
}

export type MetricsPostResult = MetricsPostOk | MetricsPostErr;

export async function handleKycMetricsPost(
  request: Request,
): Promise<MetricsPostResult> {
  let body: unknown;
  try {
    body = await request.json();
  } catch {
    return { status: 400, body: { error: "invalid_json" } };
  }
  const result = validateKycMetricEvent(body);
  if ("error" in result) {
    return { status: 400, body: { error: result.error } };
  }
  getProcessKycMetrics().record(result);
  return { status: 204 };
}

export interface MetricsGetResult {
  status: 200;
  body: string;
  headers: Record<string, string>;
}

export function handleKycMetricsGet(): MetricsGetResult {
  return {
    status: 200,
    body: getProcessKycMetrics().registry.exposition(),
    headers: {
      "content-type": "text/plain; version=0.0.4; charset=utf-8",
    },
  };
}
