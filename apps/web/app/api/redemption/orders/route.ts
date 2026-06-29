import { NextResponse } from "next/server";
import { getUpstreamSessionToken } from "@/lib/session";

const DEFAULT_API_GATEWAY_BASE_URL = "http://127.0.0.1:8080";

interface CreateRedemptionOrderPayload {
  skuId?: unknown;
  idempotencyKey?: unknown;
  correlationId?: unknown;
}

export const runtime = "nodejs";
export const dynamic = "force-dynamic";

export async function POST(request: Request): Promise<NextResponse> {
  const auth = await buildAuthHeaders();
  if (!auth) {
    return NextResponse.json({ error: "unauthorized" }, { status: 401 });
  }

  const body = await readPayload(request);
  if (!body || typeof body !== "object") {
    return NextResponse.json(
      { error: "invalid_payload", message: "Request body must be JSON" },
      { status: 400 },
    );
  }

  const skuId = parseString((body as Record<string, unknown>).skuId);
  if (!skuId) {
    return NextResponse.json(
      {
        error: "invalid_payload",
        message: "skuId is required",
      },
      { status: 400 },
    );
  }

  try {
    const response = await fetch(apiGatewayUrl("/v1/redemption/orders"), {
      method: "POST",
      cache: "no-store",
      headers: auth.headers,
      body: JSON.stringify({
        sku_id: skuId,
        idempotency_key:
          parseString((body as Record<string, unknown>).idempotencyKey) ??
          crypto.randomUUID(),
        correlation_id: parseString(
          (body as Record<string, unknown>).correlationId,
        ),
      }),
    });

    const responsePayload = await safeReadResponsePayload(response);
    return NextResponse.json(responsePayload, { status: response.status });
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
    "content-type": "application/json",
    authorization: `Bearer ${bearerToken}`,
  };

  return { headers };
}

function parseString(value: unknown): string | undefined {
  if (typeof value === "string" && value.trim().length > 0) {
    return value.trim();
  }

  return undefined;
}

async function readPayload(request: Request): Promise<unknown | null> {
  try {
    return await request.json();
  } catch {
    return null;
  }
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
