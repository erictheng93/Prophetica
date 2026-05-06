import type { OrderBookSnapshot } from "./mock-markets.js";

const DEFAULT_MATCHING_ENGINE_BASE_URL = "http://127.0.0.1:8088";
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

export interface FetchOrderBookSnapshotOptions {
  baseUrl?: string;
  marketId: string;
  depth?: number;
  fetcher?: Fetcher;
}

export async function fetchOrderBookSnapshot({
  baseUrl = process.env.MATCHING_ENGINE_BASE_URL ?? DEFAULT_MATCHING_ENGINE_BASE_URL,
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

function mapLevel(level: ApiBookLevel) {
  return {
    priceCents: level.price_pts,
    qty: level.qty,
  };
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}
