import { NextResponse } from "next/server";
import { getUpstreamSessionToken, getViewer } from "@/lib/session";

const DEFAULT_API_GATEWAY_BASE_URL = "http://127.0.0.1:8080";

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

interface RouteContext {
  params: Promise<{ marketId: string }>;
}

interface PlaceOrderPayload {
  side: string;
  outcome: string;
  type: string;
  price_pts: number;
  quantity: number;
  time_in_force: string;
}

export async function POST(
  request: Request,
  { params }: RouteContext,
): Promise<NextResponse> {
  const { marketId } = await params;
  const payload = await readPayload(request);
  if (!payload) {
    return NextResponse.json(
      { error: "request body must be valid JSON" },
      { status: 400 },
    );
  }

  const viewer = await getViewer();
  if (!viewer?.userId) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }
  const bearerToken = await getUpstreamSessionToken();
  const idempotencyKey = request.headers.get("idempotency-key")?.trim();
  if (!idempotencyKey) {
    return NextResponse.json(
      {
        error: "idempotency_key_required",
        message: "Idempotency-Key header must not be empty",
      },
      { status: 400 },
    );
  }

  const headers = new Headers({
    accept: "application/json",
    "content-type": "application/json",
  });
  headers.set("idempotency-key", idempotencyKey);
  if (bearerToken) {
    headers.set("authorization", `Bearer ${bearerToken}`);
  } else {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  try {
    const response = await fetch(apiGatewayOrderUrl(marketId), {
      method: "POST",
      cache: "no-store",
      headers,
      body: JSON.stringify(payload),
    });
    const responsePayload = await readResponsePayload(response);

    return NextResponse.json(responsePayload, { status: response.status });
  } catch (error) {
    return NextResponse.json(
      {
        error: "api_gateway_unavailable",
        message:
          error instanceof Error ? error.message : "api-gateway unavailable",
      },
      { status: 503 },
    );
  }
}

function apiGatewayOrderUrl(marketId: string): string {
  const baseUrl = trimTrailingSlash(
    process.env.API_GATEWAY_BASE_URL ?? DEFAULT_API_GATEWAY_BASE_URL,
  );
  return `${baseUrl}/v1/markets/${encodeURIComponent(marketId)}/orders`;
}

async function readResponsePayload(response: Response): Promise<unknown> {
  const text = await response.text();
  if (!text) {
    return {};
  }

  try {
    return JSON.parse(text) as unknown;
  } catch {
    return { error: text, message: text };
  }
}

async function readPayload(
  request: Request,
): Promise<PlaceOrderPayload | null> {
  try {
    return (await request.json()) as PlaceOrderPayload;
  } catch {
    return null;
  }
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}
