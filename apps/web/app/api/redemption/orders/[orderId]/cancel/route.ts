import { NextResponse } from "next/server";
import { getUpstreamSessionToken } from "@/lib/session";

const DEFAULT_API_GATEWAY_BASE_URL = "http://127.0.0.1:8080";

interface RouteContext {
  params: Promise<{ orderId: string }>;
}

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function POST(
  _request: Request,
  { params }: RouteContext,
): Promise<NextResponse> {
  const auth = await buildAuthHeaders();
  if (!auth) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  const { orderId } = await params;
  const trimmedOrderId = parsePathSegment(orderId);
  if (!trimmedOrderId) {
    return NextResponse.json(
      { error: "invalid_order_id", message: "order_id is required" },
      { status: 400 },
    );
  }

  try {
    const response = await fetch(
      apiGatewayUrl(
        `/v1/redemption/orders/${encodeURIComponent(trimmedOrderId)}/cancel`,
      ),
      {
        method: "POST",
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

async function buildAuthHeaders(): Promise<{
  headers: Record<string, string>;
} | null> {
  const bearerToken = await getUpstreamSessionToken();
  if (!bearerToken) {
    return null;
  }

  const headers: Record<string, string> = {
    accept: "application/json",
    authorization: `Bearer ${bearerToken}`,
  };

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
