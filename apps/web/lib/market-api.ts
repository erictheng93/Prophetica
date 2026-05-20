import {
  getMarketById,
  listNewest,
  listOpenMarkets,
  listSettled,
  listTrending,
  type Market,
  type MarketCategory,
  type MarketStatus,
} from "./mock-markets.js";

const DEFAULT_API_GATEWAY_BASE_URL = "http://127.0.0.1:8080";

export type MarketApiSource =
  | "api-gateway"
  | "fallback:api-gateway-unavailable";

export interface MarketListResult {
  markets: Market[];
  source: MarketApiSource;
}

export interface MarketResult {
  market: Market | undefined;
  source: MarketApiSource;
}

interface ApiMarket {
  id: string;
  title: string;
  brief: string;
  category: string;
  status: string;
  yes_price_cents: number;
  yes_bid_cents: number;
  yes_ask_cents: number;
  volume_24h_points: number;
  volume_total_points: number;
  opened_at: string;
  close_at: string;
  oracle_source: string;
  resolved_outcome: string | null;
  resolved_at: string | null;
}

interface ApiMarketList {
  markets: ApiMarket[];
}

type Fetcher = (input: string | URL, init?: RequestInit) => Promise<Response>;

interface MarketApiOptions {
  baseUrl?: string;
  fetcher?: Fetcher;
}

export async function fetchOpenMarkets(
  options: MarketApiOptions = {},
): Promise<MarketListResult> {
  return fetchMarketList({
    ...options,
    status: "open",
    fallback: listOpenMarkets,
  });
}

export async function fetchTrendingMarkets(
  options: MarketApiOptions = {},
): Promise<MarketListResult> {
  const result = await fetchMarketList({
    ...options,
    status: "open",
    fallback: listTrending,
  });
  return {
    ...result,
    markets:
      result.source === "api-gateway"
        ? [...result.markets]
            .sort((a, b) => b.volume24hPoints - a.volume24hPoints)
            .slice(0, 6)
        : result.markets,
  };
}

export async function fetchNewestMarkets(
  options: MarketApiOptions = {},
): Promise<MarketListResult> {
  const result = await fetchMarketList({
    ...options,
    status: "open",
    fallback: listNewest,
  });
  return {
    ...result,
    markets:
      result.source === "api-gateway"
        ? [...result.markets]
            .sort((a, b) => b.openedAt.localeCompare(a.openedAt))
            .slice(0, 6)
        : result.markets,
  };
}

export async function fetchSettledMarkets(
  options: MarketApiOptions = {},
): Promise<MarketListResult> {
  return fetchMarketList({
    ...options,
    status: "settled",
    fallback: listSettled,
  });
}

export async function fetchMarketById(
  id: string,
  {
    baseUrl = process.env.API_GATEWAY_BASE_URL ?? DEFAULT_API_GATEWAY_BASE_URL,
    fetcher = fetch,
  }: MarketApiOptions = {},
): Promise<MarketResult> {
  const url = new URL(
    `/v1/markets/${encodeURIComponent(id)}`,
    trimTrailingSlash(baseUrl) + "/",
  );

  try {
    const response = await fetcher(url, {
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    if (!response.ok) {
      throw new Error(`api-gateway request failed: ${response.status}`);
    }

    return {
      market: mapMarket((await response.json()) as ApiMarket),
      source: "api-gateway",
    };
  } catch {
    return {
      market: getMarketById(id),
      source: "fallback:api-gateway-unavailable",
    };
  }
}

interface FetchMarketListOptions extends MarketApiOptions {
  status?: MarketStatus;
  fallback: () => Market[];
}

async function fetchMarketList({
  baseUrl = process.env.API_GATEWAY_BASE_URL ?? DEFAULT_API_GATEWAY_BASE_URL,
  fetcher = fetch,
  status,
  fallback,
}: FetchMarketListOptions): Promise<MarketListResult> {
  const url = new URL("/v1/markets", trimTrailingSlash(baseUrl) + "/");
  if (status) {
    url.searchParams.set("status", status);
  }

  try {
    const response = await fetcher(url, {
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    if (!response.ok) {
      throw new Error(`api-gateway request failed: ${response.status}`);
    }

    const payload = (await response.json()) as ApiMarketList;
    return {
      markets: payload.markets.map(mapMarket),
      source: "api-gateway",
    };
  } catch {
    return {
      markets: fallback(),
      source: "fallback:api-gateway-unavailable",
    };
  }
}

function mapMarket(market: ApiMarket): Market {
  const mapped: Market = {
    id: market.id,
    title: market.title,
    brief: market.brief,
    category: market.category as MarketCategory,
    yesPriceCents: market.yes_price_cents,
    yesBidCents: market.yes_bid_cents,
    yesAskCents: market.yes_ask_cents,
    volume24hPoints: market.volume_24h_points,
    volumeTotalPoints: market.volume_total_points,
    openedAt: market.opened_at,
    closeAt: market.close_at,
    status: market.status as MarketStatus,
    oracleSource: market.oracle_source,
  };
  if (market.resolved_outcome === "YES" || market.resolved_outcome === "NO") {
    mapped.resolvedOutcome = market.resolved_outcome;
  }
  if (market.resolved_at) {
    mapped.resolvedAt = market.resolved_at;
  }
  return mapped;
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}
