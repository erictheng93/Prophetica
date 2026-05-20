import type { OrderBookSnapshot } from "./mock-markets.js";

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

interface ApiOrderActionResponse {
  order_id: string;
  status: string;
  filled_qty: number;
  avg_price_pts: number;
  remaining_qty: number;
  fee_pts: number;
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
  userId,
  bearerToken,
  side,
  outcome,
  type,
  pricePts,
  quantity,
  timeInForce,
  clientOrderId,
  fetcher = fetch,
}: PlaceMarketOrderOptions): Promise<PlacedMarketOrder> {
  const headers = new Headers({
    accept: "application/json",
    "content-type": "application/json",
  });
  if (bearerToken) {
    headers.set("authorization", `Bearer ${bearerToken}`);
  } else if (userId) {
    headers.set("x-user-id", userId);
  }
  if (clientOrderId) {
    headers.set("x-client-order-id", clientOrderId);
  }

  const response = await fetcher(
    `${trimTrailingSlash(orderProxyBaseUrl)}/markets/${encodeURIComponent(marketId)}/orders`,
    {
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
    },
  );
  const payload = await readJson(response);

  if (!response.ok) {
    const error =
      isRecord(payload) && typeof payload.error === "string"
        ? payload.error
        : `order request failed: ${response.status}`;
    throw new Error(error);
  }

  return mapPlacedOrder(payload as ApiOrderActionResponse);
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
