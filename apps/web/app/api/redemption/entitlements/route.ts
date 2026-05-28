import { NextResponse } from "next/server";
import { devBffFallbackEnabled } from "@/lib/dev-bff-flag";
import { getUpstreamSessionToken, getViewer } from "@/lib/session";

const DEFAULT_API_GATEWAY_BASE_URL = "http://127.0.0.1:8080";
const DEFAULT_REDEMPTION_DEV_USER_ID = "user_01";
const TRANSITION_CALLER_HEADER = "x-transition-caller";
const APPS_WEB_DEV_BFF_TRANSITION_CALLER = "apps-web-dev-bff";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function GET(): Promise<NextResponse> {
  const auth = await buildAuthHeaders();
  if (!auth) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  try {
    const response = await fetch(
      apiGatewayUrl(`/v1/redemption/entitlements/${encodeURIComponent(auth.userId)}`),
      {
        cache: "no-store",
        headers: auth.headers,
      },
    );

    const payload = await response.text();
    const parsed = parseJson(payload);
    return NextResponse.json(parsed, { status: response.status });
  } catch {
    return NextResponse.json(
      { error: "redemption_entitlements_unavailable", message: "Service unavailable" },
      { status: 503 },
    );
  }
}

async function buildAuthHeaders(): Promise<
  { userId: string; headers: Record<string, string> } | null
> {
  const bearerToken = await getUpstreamSessionToken();
  const allowDevFallback = devBffFallbackEnabled();
  const viewer = await getViewer();
  const userId =
    viewer?.userId ??
    (allowDevFallback
      ? process.env.PROPHETICA_DEV_USER_ID ?? DEFAULT_REDEMPTION_DEV_USER_ID
      : null);

  if (!bearerToken && !userId) {
    return null;
  }

  const headers: Record<string, string> = { accept: "application/json" };
  if (bearerToken) {
    headers.authorization = `Bearer ${bearerToken}`;
  } else {
    headers["x-user-id"] = userId as string;
    headers[TRANSITION_CALLER_HEADER] = APPS_WEB_DEV_BFF_TRANSITION_CALLER;
  }

  return { userId: userId as string, headers };
}

function apiGatewayUrl(path: string): string {
  return `${trimTrailingSlash(
    process.env.API_GATEWAY_BASE_URL ?? DEFAULT_API_GATEWAY_BASE_URL,
  )}${path}`;
}

function parseJson(text: string): unknown {
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return { message: text };
  }
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}
