// SPEC-005a §7.3 — POST endpoint for client-side KYC metric emission.
//
// Runtime: nodejs (not edge). The process-singleton registry needs module
// state to survive across requests inside one instance; multi-instance
// deployments scrape each instance separately.
//
// Logic lives in lib/telemetry/kyc-metrics-handler.ts so tests can target
// it without compiling next/server into the node:test runner.

import { NextResponse } from "next/server";

import { handleKycMetricsPost } from "@/lib/telemetry/kyc-metrics-handler";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function POST(request: Request): Promise<NextResponse> {
  const result = await handleKycMetricsPost(request);
  if (result.status === 204) {
    return new NextResponse(null, { status: 204 });
  }
  return NextResponse.json(result.body, { status: result.status });
}
