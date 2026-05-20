import assert from "node:assert/strict";
import { test } from "node:test";

import { fetchOrderBookSnapshot, placeMarketOrder } from "../lib/engine-api.js";

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

test("placeMarketOrder posts to the same-origin engine proxy and maps response", async () => {
  const calls: Array<{
    url: string;
    method?: string;
    headers: Record<string, string>;
    body: unknown;
  }> = [];
  const order = await placeMarketOrder({
    marketId: "mkt-election",
    userId: "42",
    side: "buy",
    outcome: "YES",
    type: "LIMIT",
    pricePts: 55,
    quantity: 3,
    timeInForce: "GTC",
    clientOrderId: "client-01",
    fetcher: async (input, init) => {
      calls.push({
        url: String(input),
        method: init?.method,
        headers: Object.fromEntries(new Headers(init?.headers).entries()),
        body: JSON.parse(String(init?.body)),
      });

      return Response.json({
        order_id: "ord_01HZ0000000000000000000000",
        status: "ACTIVE",
        filled_qty: 0,
        avg_price_pts: 0,
        remaining_qty: 3,
        fee_pts: 0,
      });
    },
  });

  assert.equal(calls[0]?.url, "/api/engine/markets/mkt-election/orders");
  assert.equal(calls[0]?.method, "POST");
  assert.equal(calls[0]?.headers["x-user-id"], "42");
  assert.equal(calls[0]?.headers["x-client-order-id"], "client-01");
  assert.deepEqual(calls[0]?.body, {
    side: "buy",
    outcome: "YES",
    type: "LIMIT",
    price_pts: 55,
    quantity: 3,
    time_in_force: "GTC",
  });
  assert.deepEqual(order, {
    orderId: "ord_01HZ0000000000000000000000",
    status: "ACTIVE",
    filledQty: 0,
    avgPricePts: 0,
    remainingQty: 3,
    feePts: 0,
  });
});

test("placeMarketOrder forwards user-service bearer token when available", async () => {
  const calls: Array<{ headers: Record<string, string> }> = [];
  await placeMarketOrder({
    marketId: "mkt-election",
    userId: "42",
    bearerToken: "jwt-user-service",
    side: "buy",
    outcome: "YES",
    type: "LIMIT",
    pricePts: 55,
    quantity: 3,
    timeInForce: "GTC",
    fetcher: async (_input, init) => {
      calls.push({
        headers: Object.fromEntries(new Headers(init?.headers).entries()),
      });

      return Response.json({
        order_id: "ord_01HZ0000000000000000000000",
        status: "ACTIVE",
        filled_qty: 0,
        avg_price_pts: 0,
        remaining_qty: 3,
        fee_pts: 0,
      });
    },
  });

  assert.equal(calls[0]?.headers.authorization, "Bearer jwt-user-service");
  assert.equal(calls[0]?.headers["x-user-id"], undefined);
});

test("placeMarketOrder exposes backend rejection messages", async () => {
  await assert.rejects(
    () =>
      placeMarketOrder({
        marketId: "mkt-election",
        userId: "42",
        side: "buy",
        outcome: "YES",
        type: "LIMIT",
        pricePts: 55,
        quantity: 3,
        timeInForce: "GTC",
        fetcher: async () =>
          Response.json({ error: "self trade prevented" }, { status: 412 }),
      }),
    /self trade prevented/,
  );
});
