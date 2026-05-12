import { randomUUID } from "node:crypto";
import type { SessionPayload } from "./session-crypto.js";
import {
  isManualPointDirection,
  type ManualPointsAdjustmentRequest,
} from "./points.js";

type FetchLike = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>;

export interface ProxyPointsBalanceInput {
  viewer: SessionPayload | null;
  userId: string;
  ledgerUrl?: string;
  fetchImpl?: FetchLike;
}

export interface ProxyManualPointsAdjustmentInput {
  viewer: SessionPayload | null;
  request: unknown;
  ledgerUrl?: string;
  fetchImpl?: FetchLike;
}

const DEFAULT_LEDGER_URL = "http://127.0.0.1:8086";

export async function proxyPointsBalanceRequest(
  input: ProxyPointsBalanceInput,
): Promise<Response> {
  if (!canReadPoints(input.viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  const userId = input.userId.trim();
  if (!userId) {
    return Response.json({ error: "invalid_user_id" }, { status: 400 });
  }

  const upstreamPath = `/internal/ledger/wallets/${encodeURIComponent(userId)}/balance`;
  return proxyToLedger({
    method: "GET",
    path: upstreamPath,
    viewer: input.viewer,
    ledgerUrl: input.ledgerUrl,
    fetchImpl: input.fetchImpl,
  });
}

export async function proxyManualPointsAdjustment(
  input: ProxyManualPointsAdjustmentInput,
): Promise<Response> {
  if (!canAdjustPoints(input.viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  const parsed = parseManualPointsAdjustment(input.request);
  if ("error" in parsed) {
    return Response.json({ error: parsed.error }, { status: 400 });
  }
  if (input.viewer.role === "affiliate") {
    if (parsed.value.direction !== "grant") {
      return Response.json({ error: "forbidden_direction" }, { status: 403 });
    }
    if (!input.viewer.affiliateId) {
      return Response.json({ error: "affiliate_id_required" }, { status: 403 });
    }
  }

  const correlationId =
    parsed.value.correlation_id?.trim() || `admin-points:${randomUUID()}`;
  const idempotencyKey =
    parsed.value.idempotency_key?.trim() ||
    `manual-points:${parsed.value.direction}:${randomUUID()}`;
  const body = JSON.stringify({
    actor_user_id: input.viewer.sub,
    actor_role: input.viewer.role,
    affiliate_id:
      input.viewer.role === "affiliate" ? input.viewer.affiliateId : undefined,
    target_user_id: parsed.value.target_user_id,
    direction: parsed.value.direction,
    delta_minor_units: parsed.value.delta_minor_units,
    reason_code: parsed.value.reason_code,
    reason_text: parsed.value.reason_text,
    correlation_id: correlationId,
  });

  return proxyToLedger({
    method: "POST",
    path: "/internal/ledger/manual-points-adjustments",
    viewer: input.viewer,
    body,
    headers: {
      "content-type": "application/json",
      "idempotency-key": idempotencyKey,
      "x-correlation-id": correlationId,
    },
    ledgerUrl: input.ledgerUrl,
    fetchImpl: input.fetchImpl,
  });
}

function canReadPoints(
  viewer: SessionPayload | null,
): viewer is SessionPayload {
  return (
    viewer?.role === "admin" ||
    viewer?.role === "manager" ||
    viewer?.role === "affiliate"
  );
}

function canAdjustPoints(
  viewer: SessionPayload | null,
): viewer is SessionPayload {
  return (
    viewer?.role === "admin" ||
    viewer?.role === "manager" ||
    viewer?.role === "affiliate"
  );
}

function parseManualPointsAdjustment(
  value: unknown,
): { value: ManualPointsAdjustmentRequest } | { error: string } {
  if (!value || typeof value !== "object") {
    return { error: "invalid_request" };
  }
  const candidate = value as Record<string, unknown>;
  const direction = candidate.direction;
  if (!isManualPointDirection(direction)) {
    return { error: "invalid_direction" };
  }
  if (
    typeof candidate.target_user_id !== "string" ||
    candidate.target_user_id.trim() === ""
  ) {
    return { error: "invalid_target_user_id" };
  }
  if (
    typeof candidate.delta_minor_units !== "number" ||
    !Number.isSafeInteger(candidate.delta_minor_units) ||
    candidate.delta_minor_units <= 0
  ) {
    return { error: "invalid_delta_minor_units" };
  }
  if (
    typeof candidate.reason_code !== "string" ||
    candidate.reason_code.trim() === ""
  ) {
    return { error: "invalid_reason_code" };
  }
  if (
    typeof candidate.reason_text !== "string" ||
    candidate.reason_text.trim() === ""
  ) {
    return { error: "invalid_reason_text" };
  }

  return {
    value: {
      target_user_id: candidate.target_user_id.trim(),
      direction,
      delta_minor_units: candidate.delta_minor_units,
      reason_code: candidate.reason_code.trim(),
      reason_text: candidate.reason_text.trim(),
      correlation_id:
        typeof candidate.correlation_id === "string"
          ? candidate.correlation_id.trim()
          : undefined,
      idempotency_key:
        typeof candidate.idempotency_key === "string"
          ? candidate.idempotency_key.trim()
          : undefined,
    },
  };
}

async function proxyToLedger(input: {
  method: "GET" | "POST";
  path: string;
  viewer: SessionPayload;
  body?: string;
  headers?: Record<string, string>;
  ledgerUrl?: string;
  fetchImpl?: FetchLike;
}): Promise<Response> {
  const requestId =
    input.headers?.["x-correlation-id"] ?? `admin-points:${randomUUID()}`;
  const headers: Record<string, string> = {
    "x-actor-id": input.viewer.sub,
    "x-operator-role": input.viewer.role,
    "x-request-id": requestId,
    ...input.headers,
  };
  if (input.viewer.affiliateId) {
    headers["x-affiliate-id"] = input.viewer.affiliateId;
  }
  const upstreamUrl = new URL(
    input.path,
    input.ledgerUrl ?? process.env.WALLET_LEDGER_BASE_URL ?? DEFAULT_LEDGER_URL,
  );

  try {
    const upstream = await (input.fetchImpl ?? fetch)(upstreamUrl, {
      method: input.method,
      headers,
      body: input.body,
      cache: "no-store",
    });
    const responseHeaders = new Headers();
    const contentType = upstream.headers.get("content-type");
    if (contentType) responseHeaders.set("content-type", contentType);
    return new Response(await upstream.arrayBuffer(), {
      status: upstream.status,
      headers: responseHeaders,
    });
  } catch {
    return Response.json(
      { error: "wallet_ledger_unavailable" },
      { status: 502 },
    );
  }
}
