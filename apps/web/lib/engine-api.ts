import type { OrderBookSnapshot } from "./mock-markets.js";
import { withIdempotencyKey } from "./idempotency.js";

const DEFAULT_MATCHING_ENGINE_BASE_URL = "http://127.0.0.1:8088";
const DEFAULT_ORDER_PROXY_BASE_URL = "/api/engine";
const DEFAULT_BOOK_DEPTH = 5;

export type OrderBookSnapshotSource =
  | "matching-engine"
  | "fallback:matching-engine-unavailable";

export interface EngineOrderBookSnapshot extends OrderBookSnapshot {
  source: OrderBookSnapshotSource;
}

interface ApiBookLevel {
  price_pts: number;
  qty: number;
}

interface ApiBookSnapshot {
  market_id: string;
  bids: ApiBookLevel[];
  asks: ApiBookLevel[];
}

type Fetcher = (input: string | URL, init?: RequestInit) => Promise<Response>;

export type MarketOrderOutcome = "YES" | "NO";
export type MarketOrderSide = "buy" | "sell";
export type MarketOrderType = "LIMIT" | "IOC" | "FOK" | "POST_ONLY";
export type MarketOrderTimeInForce = "GTC" | "IOC" | "FOK";

export interface PlaceMarketOrderOptions {
  orderProxyBaseUrl?: string;
  marketId: string;
  userId?: string;
  bearerToken?: string;
  side: MarketOrderSide;
  outcome: MarketOrderOutcome;
  type: MarketOrderType;
  pricePts: number;
  quantity: number;
  timeInForce: MarketOrderTimeInForce;
  clientOrderId?: string;
  fetcher?: Fetcher;
}

export interface PlacedMarketOrder {
  orderId: string;
  status: string;
  filledQty: number;
  avgPricePts: number;
  remainingQty: number;
  feePts: number;
}

export interface QueuedMarketOrder {
  kind: "queued";
  status: "queued";
  idempotencyKey: string;
  queuedAt: number;
}

export type PlaceMarketOrderResult = PlacedMarketOrder | QueuedMarketOrder;

export class MarketOrderApiError extends Error {
  readonly status: number;
  readonly code: string;
  readonly redirectTo?: string;

  constructor(
    status: number,
    code: string,
    message: string,
    redirectTo?: string,
  ) {
    super(message);
    this.name = "MarketOrderApiError";
    this.status = status;
    this.code = code;
    this.redirectTo = redirectTo;
  }
}

interface ApiOrderActionResponse {
  order_id: string;
  status: string;
  filled_qty: number;
  avg_price_pts: number;
  remaining_qty: number;
  fee_pts: number;
}

interface ApiQueuedOrderResponse {
  status: "queued";
  idempotency_key: string;
  queued_at: number;
}

interface ApiErrorResponse {
  error?: string;
  message?: string;
  redirect_to?: string;
}

export interface FetchOrderBookSnapshotOptions {
  baseUrl?: string;
  marketId: string;
  depth?: number;
  fetcher?: Fetcher;
}

export async function fetchOrderBookSnapshot({
  baseUrl = process.env.MATCHING_ENGINE_BASE_URL ??
    DEFAULT_MATCHING_ENGINE_BASE_URL,
  marketId,
  depth = DEFAULT_BOOK_DEPTH,
  fetcher = fetch,
}: FetchOrderBookSnapshotOptions): Promise<EngineOrderBookSnapshot> {
  const url = new URL(
    `/internal/engine/markets/${encodeURIComponent(marketId)}/book`,
    trimTrailingSlash(baseUrl) + "/",
  );
  url.searchParams.set("depth", String(depth));

  try {
    const response = await fetcher(url, {
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    if (!response.ok) {
      throw new Error(`matching-engine request failed: ${response.status}`);
    }

    const snapshot = (await response.json()) as ApiBookSnapshot;
    return {
      bids: snapshot.bids.map(mapLevel),
      asks: snapshot.asks.map(mapLevel),
      source: "matching-engine",
    };
  } catch {
    return {
      bids: [],
      asks: [],
      source: "fallback:matching-engine-unavailable",
    };
  }
}

export async function placeMarketOrder({
  orderProxyBaseUrl = DEFAULT_ORDER_PROXY_BASE_URL,
  marketId,
  bearerToken,
  side,
  outcome,
  type,
  pricePts,
  quantity,
  timeInForce,
  clientOrderId,
  fetcher = fetch,
}: PlaceMarketOrderOptions): Promise<PlaceMarketOrderResult> {
  const headers = new Headers({
    accept: "application/json",
    "content-type": "application/json",
  });
  if (bearerToken) {
    headers.set("authorization", `Bearer ${bearerToken}`);
  }
  if (clientOrderId) {
    headers.set("x-client-order-id", clientOrderId);
  }

  const init = withIdempotencyKey({
    method: "POST",
    cache: "no-store",
    headers,
    body: JSON.stringify({
      side,
      outcome,
      type,
      price_pts: pricePts,
      quantity,
      time_in_force: timeInForce,
    }),
  });

  const response = await fetcher(
    `${trimTrailingSlash(orderProxyBaseUrl)}/markets/${encodeURIComponent(marketId)}/orders`,
    init,
  );
  const payload = await readJson(response);

  if (!response.ok) {
    throw marketOrderApiError(response.status, payload);
  }
  if (response.status === 202 && isQueuedOrderResponse(payload)) {
    return {
      kind: "queued",
      status: "queued",
      idempotencyKey: payload.idempotency_key,
      queuedAt: payload.queued_at,
    };
  }

  return mapPlacedOrder(payload as ApiOrderActionResponse);
}

export function orderRejectionRedirectPath(
  error: unknown,
  returnTo: string,
): string | null {
  if (!(error instanceof MarketOrderApiError)) {
    return null;
  }

  if (error.status === 401 || error.code === "unauthorized") {
    return withReturnTo("/auth/sign-in", returnTo);
  }
  if (error.code === "ERROR_CODE_KYC_TIER_INSUFFICIENT") {
    return withReturnTo(error.redirectTo ?? "/kyc/tier-1", returnTo);
  }
  if (error.code === "ERROR_CODE_INSUFFICIENT_FUNDS") {
    return withReturnTo(error.redirectTo ?? "/topup", returnTo);
  }

  return null;
}

function marketOrderApiError(
  status: number,
  payload: unknown,
): MarketOrderApiError {
  if (isRecord(payload)) {
    const body = payload as ApiErrorResponse;
    const code = typeof body.error === "string" ? body.error : `http_${status}`;
    const message =
      typeof body.message === "string"
        ? body.message
        : typeof body.error === "string"
          ? body.error
          : `order request failed: ${status}`;
    return new MarketOrderApiError(
      status,
      code,
      message,
      typeof body.redirect_to === "string" ? body.redirect_to : undefined,
    );
  }

  return new MarketOrderApiError(
    status,
    `http_${status}`,
    `order request failed: ${status}`,
  );
}

function withReturnTo(path: string, returnTo: string): string {
  const separator = path.includes("?") ? "&" : "?";
  return `${path}${separator}return_to=${encodeURIComponent(returnTo)}`;
}

function mapLevel(level: ApiBookLevel) {
  return {
    priceCents: level.price_pts,
    qty: level.qty,
  };
}

function mapPlacedOrder(order: ApiOrderActionResponse): PlacedMarketOrder {
  return {
    orderId: order.order_id,
    status: order.status,
    filledQty: order.filled_qty,
    avgPricePts: order.avg_price_pts,
    remainingQty: order.remaining_qty,
    feePts: order.fee_pts,
  };
}

function isQueuedOrderResponse(
  payload: unknown,
): payload is ApiQueuedOrderResponse {
  if (!isRecord(payload)) return false;
  return (
    payload.status === "queued" &&
    typeof payload.idempotency_key === "string" &&
    typeof payload.queued_at === "number"
  );
}

async function readJson(response: Response): Promise<unknown> {
  const text = await response.text();
  if (text.length === 0) return undefined;

  try {
    return JSON.parse(text) as unknown;
  } catch {
    return { error: text };
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}
