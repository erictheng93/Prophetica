import assert from "node:assert/strict";
import { test } from "node:test";

import { fetchPortfolioPositions } from "../lib/position-api.js";

const apiPosition = {
  id: "pos-1",
  user_id: "user_01",
  market_id: "mkt-election",
  outcome: "YES",
  qty: 100,
  avg_price_pts: 42,
  locked_amount: 4200,
  opened_at: "2026-05-01T00:00:00+08:00",
  closed_at: null,
  settle_txn_id: null,
  realized_pnl_points: null,
};

const apiMarket = {
  id: "mkt-election",
  title: "Election market",
  brief: "Will the candidate win?",
  category: "sports",
  status: "open",
  yes_price_cents: 54,
  yes_bid_cents: 53,
  yes_ask_cents: 55,
  volume_24h_points: 1200,
  volume_total_points: 9800,
  opened_at: "2026-05-01T00:00:00+08:00",
  close_at: "2026-06-01T00:00:00+08:00",
  oracle_source: "official result",
  resolved_outcome: null,
  resolved_at: null,
};

test("fetchPortfolioPositions calls position-service and resolves market values", async () => {
  const positionCalls: string[] = [];
  const marketCalls: string[] = [];

  const portfolio = await fetchPortfolioPositions({
    baseUrl: "http://position.local",
    marketBaseUrl: "http://market.local",
    userId: "user_01",
    fetcher: async (input: string | URL): Promise<Response> => {
      positionCalls.push(String(input));
      return Response.json({ positions: [apiPosition] });
    },
    marketFetcher: async (input: string | URL): Promise<Response> => {
      marketCalls.push(String(input));
      return Response.json(apiMarket);
    },
  });

  assert.equal(
    positionCalls[0],
    "http://position.local/internal/users/user_01/positions",
  );
  assert.equal(marketCalls[0], "http://market.local/v1/markets/mkt-election");
  assert.equal(portfolio.source, "position-service");
  assert.equal(portfolio.active[0]?.market.id, "mkt-election");
  assert.equal(portfolio.active[0]?.currentValuePoints, 54);
  assert.equal(portfolio.summary.totalCostPoints, 42);
  assert.equal(portfolio.summary.unrealizedPnlPoints, 12);
});

test("fetchPortfolioPositions falls back to mock portfolio when position-service is unavailable", async () => {
  const portfolio = await fetchPortfolioPositions({
    baseUrl: "http://position.local",
    fetcher: async () => {
      throw new Error("connection refused");
    },
  });

  assert.equal(portfolio.source, "fallback:position-service-unavailable");
  assert.ok(portfolio.active.length > 0);
  assert.ok(portfolio.summary.totalCostPoints > 0);
});
