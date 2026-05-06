import assert from "node:assert/strict";
import { test } from "node:test";

import {
  fetchMarketById,
  fetchOpenMarkets,
  fetchSettledMarkets,
} from "../lib/market-api.js";

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

test("fetchOpenMarkets calls market-service list facade and maps fields", async () => {
  const calls: string[] = [];
  const fetcher = async (input: string | URL): Promise<Response> => {
    calls.push(String(input));
    return Response.json({ markets: [apiMarket] });
  };

  const markets = await fetchOpenMarkets({
    baseUrl: "http://market.local",
    fetcher,
  });

  assert.equal(calls[0], "http://market.local/internal/markets?status=open");
  assert.equal(markets.source, "market-service");
  assert.deepEqual(markets.markets, [
    {
      id: "mkt-election",
      title: "Election market",
      brief: "Will the candidate win?",
      category: "sports",
      status: "open",
      yesPriceCents: 54,
      yesBidCents: 53,
      yesAskCents: 55,
      volume24hPoints: 1200,
      volumeTotalPoints: 9800,
      openedAt: "2026-05-01T00:00:00+08:00",
      closeAt: "2026-06-01T00:00:00+08:00",
      oracleSource: "official result",
    },
  ]);
});

test("fetchMarketById calls market-service get facade", async () => {
  const calls: string[] = [];
  const fetcher = async (input: string | URL): Promise<Response> => {
    calls.push(String(input));
    return Response.json(apiMarket);
  };

  const market = await fetchMarketById("mkt-election", {
    baseUrl: "http://market.local",
    fetcher,
  });

  assert.equal(calls[0], "http://market.local/internal/markets/mkt-election");
  assert.equal(market?.source, "market-service");
  assert.equal(market?.market?.id, "mkt-election");
  assert.equal(market?.market?.yesPriceCents, 54);
});

test("fetchSettledMarkets falls back to mock data when market-service is unavailable", async () => {
  const markets = await fetchSettledMarkets({
    baseUrl: "http://market.local",
    fetcher: async () => {
      throw new Error("connection refused");
    },
  });

  assert.equal(markets.source, "fallback:market-service-unavailable");
  assert.ok(markets.markets.some((market) => market.status === "settled"));
});
