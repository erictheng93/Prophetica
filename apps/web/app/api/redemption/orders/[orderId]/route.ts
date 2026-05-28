import { NextResponse } from "next/server";
import { devBffFallbackEnabled } from "@/lib/dev-bff-flag";
import { getUpstreamSessionToken, getViewer } from "@/lib/session";

const DEFAULT_API_GATEWAY_BASE_URL = "http://127.0.0.1:8080";
const DEFAULT_REDEMPTION_DEV_USER_ID = "user_01";
const TRANSITION_CALLER_HEADER = "x-transition-caller";
const APPS_WEB_DEV_BFF_TRANSITION_CALLER = "apps-web-dev-bff";

interface RouteContext {
  params: Promise<{ orderId: string }>;
}

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function GET(
  _request: Request,
  { params }: RouteContext,
): Promise<NextResponse> {
  const auth = await buildAuthHeaders();
  if (!auth) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  const { orderId } = await params;
  const orderIdValue = parsePathSegment(orderId);
  if (!orderIdValue) {
    return NextResponse.json(
      { error: "invalid_order_id", message: "order_id is required" },
      { status: 400 },
    );
  }

  try {
    const response = await fetch(
      apiGatewayUrl(`/v1/redemption/orders/${encodeURIComponent(orderIdValue)}`),
      {
        cache: "no-store",
        headers: auth.headers,
      },
    );

    const payload = await safeReadResponsePayload(response);
    return NextResponse.json(payload, { status: response.status });
  } catch {
    return NextResponse.json(
      {
        error: "api_gateway_unavailable",
        message: "redemption service is unavailable",
      },
      { status: 503 },
    );
  }
}

async function buildAuthHeaders(): Promise<
  { headers: Record<string, string> } | null
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

  return { headers };
}

function parsePathSegment(value: string): string | null {
  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function apiGatewayUrl(path: string): string {
  return `${trimTrailingSlash(
    process.env.API_GATEWAY_BASE_URL ?? DEFAULT_API_GATEWAY_BASE_URL,
  )}${path}`;
}

async function safeReadResponsePayload(response: Response): Promise<unknown> {
  const text = await response.text();
  try {
    return JSON.parse(text) as unknown;
  } catch {
    return { error: "redemption_request_failed", message: text };
  }
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}
