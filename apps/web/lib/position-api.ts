import {
  listActivePositions,
  listSettledPositions,
  summarizePortfolio,
  type PortfolioSummary,
  type Position,
  type ResolvedPosition,
} from "./mock-portfolio.js";
import { fetchMarketById } from "./market-api.js";
import type { Market } from "./mock-markets.js";

const DEFAULT_POSITION_SERVICE_BASE_URL = "http://127.0.0.1:8089";
const DEFAULT_PORTFOLIO_USER_ID = "user_01";

export type PortfolioPositionsSource =
  | "position-service"
  | "fallback:position-service-unavailable";

export interface PortfolioPositions {
  active: ResolvedPosition[];
  settled: ResolvedPosition[];
  summary: PortfolioSummary;
  source: PortfolioPositionsSource;
}

interface ApiPosition {
  id: string;
  user_id: string;
  market_id: string;
  outcome: string;
  qty: number;
  avg_price_pts: number;
  locked_amount: number;
  opened_at: string;
  closed_at: string | null;
  settle_txn_id: number | null;
  realized_pnl_points: number | null;
}

interface ApiPositionList {
  positions: ApiPosition[];
}

type Fetcher = (input: string | URL, init?: RequestInit) => Promise<Response>;

interface FetchPortfolioPositionsOptions {
  baseUrl?: string;
  marketBaseUrl?: string;
  userId?: string;
  fetcher?: Fetcher;
  marketFetcher?: Fetcher;
}

export async function fetchPortfolioPositions({
  baseUrl = process.env.POSITION_SERVICE_BASE_URL ?? DEFAULT_POSITION_SERVICE_BASE_URL,
  marketBaseUrl,
  userId = DEFAULT_PORTFOLIO_USER_ID,
  fetcher = fetch,
  marketFetcher,
}: FetchPortfolioPositionsOptions = {}): Promise<PortfolioPositions> {
  const url = new URL(`/internal/users/${encodeURIComponent(userId)}/positions`, trimTrailingSlash(baseUrl) + "/");

  try {
    const response = await fetcher(url, {
      cache: "no-store",
      headers: { accept: "application/json" },
    });
    if (!response.ok) {
      throw new Error(`position-service request failed: ${response.status}`);
    }

    const payload = (await response.json()) as ApiPositionList;
    const positions = payload.positions.map(mapPosition);
    const resolved = await resolvePositions(positions, { marketBaseUrl, marketFetcher });

    return {
      active: resolved.filter((position) => !position.settledAt),
      settled: resolved.filter((position) => !!position.settledAt),
      summary: summarizeResolvedPositions(resolved),
      source: "position-service",
    };
  } catch {
    return {
      active: listActivePositions(),
      settled: listSettledPositions(),
      summary: summarizePortfolio(),
      source: "fallback:position-service-unavailable",
    };
  }
}

function mapPosition(position: ApiPosition): Position {
  const mapped: Position = {
    marketId: position.market_id,
    outcome: position.outcome === "NO" ? "NO" : "YES",
    shares: position.qty,
    avgCostCents: position.avg_price_pts,
  };
  if (position.closed_at) {
    mapped.settledAt = position.closed_at;
  }
  if (position.realized_pnl_points !== null) {
    mapped.realizedPnlPoints = position.realized_pnl_points;
  }
  return mapped;
}

async function resolvePositions(
  positions: Position[],
  {
    marketBaseUrl,
    marketFetcher,
  }: Pick<FetchPortfolioPositionsOptions, "marketBaseUrl" | "marketFetcher">,
): Promise<ResolvedPosition[]> {
  const resolved = await Promise.all(
    positions.map(async (position) => {
      const { market } = await fetchMarketById(position.marketId, {
        baseUrl: marketBaseUrl,
        fetcher: marketFetcher,
      });
      return market ? resolvePosition(position, market) : null;
    }),
  );

  return resolved.filter((position): position is ResolvedPosition => position !== null);
}

function resolvePosition(position: Position, market: Market): ResolvedPosition {
  const totalCostPoints = (position.shares * position.avgCostCents) / 100;
  const currentValuePoints =
    (position.shares * currentPriceCentsForOutcome(market, position.outcome)) / 100;
  const unrealizedPnlPoints = currentValuePoints - totalCostPoints;
  const unrealizedPnlPct =
    totalCostPoints > 0 ? (unrealizedPnlPoints / totalCostPoints) * 100 : 0;

  return {
    ...position,
    market,
    totalCostPoints,
    currentValuePoints,
    unrealizedPnlPoints,
    unrealizedPnlPct,
  };
}

function summarizeResolvedPositions(positions: ResolvedPosition[]): PortfolioSummary {
  const active = positions.filter((position) => !position.settledAt);
  const settled = positions.filter((position) => !!position.settledAt);

  return {
    totalCostPoints: active.reduce((sum, position) => sum + position.totalCostPoints, 0),
    currentValuePoints: active.reduce((sum, position) => sum + position.currentValuePoints, 0),
    unrealizedPnlPoints: active.reduce((sum, position) => sum + position.unrealizedPnlPoints, 0),
    realizedPnlPointsLifetime: settled.reduce(
      (sum, position) => sum + (position.realizedPnlPoints ?? 0),
      0,
    ),
  };
}

function currentPriceCentsForOutcome(market: Market, outcome: "YES" | "NO"): number {
  return outcome === "YES" ? market.yesPriceCents : 100 - market.yesPriceCents;
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}
