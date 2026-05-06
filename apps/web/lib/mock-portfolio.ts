import { getMarketById, type Market } from "./mock-markets.js";

// Mock-only player portfolio. Replaced by position-service / wallet-ledger
// snapshot via packages/ts-sdk once the proto + RPC land.
//
// Position model aligns with SPEC glossary (SPEC-000 §3):
//   - Position = 玩家在某個 Outcome 上的累計持有股數
//   - shares × outcome price (cents) = current notional in 點
//   - avg_entry_price stored in cents (SPEC-003 §3 unit)

export interface Position {
  marketId: string;
  outcome: "YES" | "NO";
  shares: number;
  avgCostCents: number;
  /** Set on settled-market positions; cents-domain → 點 already converted. */
  settledAt?: string;
  realizedPnlPoints?: number;
}

export interface ResolvedPosition extends Position {
  market: Market;
  totalCostPoints: number;
  /** For active positions only — settled positions show realizedPnlPoints. */
  currentValuePoints: number;
  unrealizedPnlPoints: number;
  unrealizedPnlPct: number;
}

export const mockPositions: Position[] = [
  { marketId: "wbsc-2026-tw-semifinal", outcome: "YES", shares: 100, avgCostCents: 32 },
  { marketId: "fomc-2026-06-rate-cut", outcome: "NO", shares: 80, avgCostCents: 38 },
  { marketId: "tpe-2026-07-temp-30", outcome: "YES", shares: 50, avgCostCents: 75 },
  { marketId: "wwdc-2026-vision-pro", outcome: "YES", shares: 200, avgCostCents: 28 },
  {
    marketId: "nobel-2025-literature-korean",
    outcome: "YES",
    shares: 60,
    avgCostCents: 45,
    settledAt: "2025-10-09",
    // Bought YES at 45¢ × 60 shares = 27 點 cost; settled YES → 60 × 100 / 100 = 60 點 received → +33 點.
    realizedPnlPoints: 33,
  },
  {
    marketId: "us-jobs-2026-05-200k",
    outcome: "YES",
    shares: 40,
    avgCostCents: 60,
    settledAt: "2026-06-06",
    // Bought YES at 60¢ × 40 = 24 點; settled NO → 0 received → −24 點.
    realizedPnlPoints: -24,
  },
];

function currentPriceCentsForOutcome(market: Market, outcome: "YES" | "NO"): number {
  return outcome === "YES" ? market.yesPriceCents : 100 - market.yesPriceCents;
}

function resolve(p: Position): ResolvedPosition | null {
  const market = getMarketById(p.marketId);
  if (!market) return null;
  const totalCostPoints = (p.shares * p.avgCostCents) / 100;
  const currentValuePoints = (p.shares * currentPriceCentsForOutcome(market, p.outcome)) / 100;
  const unrealizedPnlPoints = currentValuePoints - totalCostPoints;
  const unrealizedPnlPct = totalCostPoints > 0 ? (unrealizedPnlPoints / totalCostPoints) * 100 : 0;
  return {
    ...p,
    market,
    totalCostPoints,
    currentValuePoints,
    unrealizedPnlPoints,
    unrealizedPnlPct,
  };
}

export function listActivePositions(): ResolvedPosition[] {
  return mockPositions
    .filter((p) => !p.settledAt)
    .map(resolve)
    .filter((p): p is ResolvedPosition => p !== null);
}

export function listSettledPositions(): ResolvedPosition[] {
  return mockPositions
    .filter((p) => !!p.settledAt)
    .map(resolve)
    .filter((p): p is ResolvedPosition => p !== null);
}

export interface PortfolioSummary {
  totalCostPoints: number;
  currentValuePoints: number;
  unrealizedPnlPoints: number;
  realizedPnlPointsLifetime: number;
}

export function summarizePortfolio(): PortfolioSummary {
  const active = listActivePositions();
  const settled = listSettledPositions();
  return {
    totalCostPoints: active.reduce((s, p) => s + p.totalCostPoints, 0),
    currentValuePoints: active.reduce((s, p) => s + p.currentValuePoints, 0),
    unrealizedPnlPoints: active.reduce((s, p) => s + p.unrealizedPnlPoints, 0),
    realizedPnlPointsLifetime: settled.reduce((s, p) => s + (p.realizedPnlPoints ?? 0), 0),
  };
}

/**
 * Round a 點 number to a sane display precision. Internal cent-domain math
 * can produce arbitrary fractions (e.g. 1.5 點); UI shows up to 2 decimals.
 */
export function formatPointsSigned(n: number): string {
  const sign = n > 0 ? "+" : n < 0 ? "−" : "";
  const abs = Math.abs(n);
  const formatted = abs.toLocaleString("zh-Hant", {
    minimumFractionDigits: abs % 1 === 0 ? 0 : 2,
    maximumFractionDigits: 2,
  });
  return `${sign}${formatted} 點`;
}
