import assert from "node:assert/strict";
import { test } from "node:test";

import { fetchOrderBookSnapshot } from "../lib/engine-api.js";

test("fetchOrderBookSnapshot calls matching-engine book facade and maps levels", async () => {
  const calls: string[] = [];
  const fetcher = async (input: string | URL): Promise<Response> => {
    calls.push(String(input));
    return Response.json({
      market_id: "mkt-election",
      bids: [{ price_pts: 54, qty: 7 }],
      asks: [{ price_pts: 57, qty: 2 }],
    });
  };

  const book = await fetchOrderBookSnapshot({
    baseUrl: "http://engine.local",
    marketId: "mkt-election",
    depth: 5,
    fetcher,
  });

  assert.equal(
    calls[0],
    "http://engine.local/internal/engine/markets/mkt-election/book?depth=5",
  );
  assert.deepEqual(book.bids, [{ priceCents: 54, qty: 7 }]);
  assert.deepEqual(book.asks, [{ priceCents: 57, qty: 2 }]);
  assert.equal(book.source, "matching-engine");
});

test("fetchOrderBookSnapshot returns an empty fallback when matching-engine is unavailable", async () => {
  const book = await fetchOrderBookSnapshot({
    baseUrl: "http://engine.local",
    marketId: "mkt-election",
    fetcher: async () => {
      throw new Error("connection refused");
    },
  });

  assert.deepEqual(book.bids, []);
  assert.deepEqual(book.asks, []);
  assert.equal(book.source, "fallback:matching-engine-unavailable");
});
