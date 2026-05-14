import "server-only";

import { headers } from "next/headers";
import type { AnalyticsFetchOptions } from "./analytics-reporting.js";

export async function analyticsServerFetchOptions(): Promise<AnalyticsFetchOptions> {
  const incomingHeaders = await headers();
  const host =
    firstHeaderValue(incomingHeaders.get("x-forwarded-host")) ??
    firstHeaderValue(incomingHeaders.get("host"));
  if (!host) {
    throw new Error("missing request host for analytics bff fetch");
  }

  const proto =
    firstHeaderValue(incomingHeaders.get("x-forwarded-proto")) ??
    (host.startsWith("localhost") || host.startsWith("127.0.0.1")
      ? "http"
      : "https");
  const cookie = incomingHeaders.get("cookie");
  const internalMtls = incomingHeaders.get("x-internal-mtls");
  const forwardedHeaders: Record<string, string> = {};

  if (cookie) forwardedHeaders.cookie = cookie;
  if (internalMtls) forwardedHeaders["x-internal-mtls"] = internalMtls;

  return {
    baseUrl: `${proto}://${host}`,
    headers:
      Object.keys(forwardedHeaders).length > 0 ? forwardedHeaders : undefined,
  };
}

function firstHeaderValue(value: string | null): string | undefined {
  return value?.split(",")[0]?.trim() || undefined;
}
