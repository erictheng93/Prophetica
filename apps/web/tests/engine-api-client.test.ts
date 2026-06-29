import assert from "node:assert/strict";
import { test } from "node:test";

import {
  MarketOrderApiError,
  fetchOrderBookSnapshot,
  orderRejectionRedirectPath,
  placeMarketOrder,
} from "../lib/engine-api.js";

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

test("placeMarketOrder posts with idempotency to the same-origin order proxy and maps response", async () => {
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
  assert.equal(calls[0]?.headers["x-user-id"], undefined);
  assert.equal(calls[0]?.headers["x-client-order-id"], "client-01");
  assert.match(
    calls[0]?.headers["idempotency-key"] ?? "",
    /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/,
  );
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

test("placeMarketOrder maps service-worker queued responses", async () => {
  const order = await placeMarketOrder({
    marketId: "mkt-election",
    side: "buy",
    outcome: "YES",
    type: "LIMIT",
    pricePts: 55,
    quantity: 3,
    timeInForce: "GTC",
    fetcher: async () =>
      Response.json(
        {
          status: "queued",
          idempotency_key: "idem-order-queued",
          queued_at: 1_800_000_000_000,
        },
        { status: 202 },
      ),
  });

  assert.deepEqual(order, {
    kind: "queued",
    status: "queued",
    idempotencyKey: "idem-order-queued",
    queuedAt: 1_800_000_000_000,
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

test("placeMarketOrder exposes structured gateway rejection metadata", async () => {
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
          Response.json(
            {
              error: "ERROR_CODE_KYC_TIER_INSUFFICIENT",
              message: "KYC tier is below the minimum required for trading",
              redirect_to: "/kyc/tier-1",
            },
            { status: 403 },
          ),
      }),
    (error: unknown) =>
      error instanceof MarketOrderApiError &&
      error.status === 403 &&
      error.code === "ERROR_CODE_KYC_TIER_INSUFFICIENT" &&
      error.redirectTo === "/kyc/tier-1",
  );
});

test("orderRejectionRedirectPath maps SPEC-023 order gates to return_to redirects", () => {
  assert.equal(
    orderRejectionRedirectPath(
      new MarketOrderApiError(401, "unauthorized", "unauthorized"),
      "/markets/mkt-election?tab=order",
    ),
    "/auth/sign-in?return_to=%2Fmarkets%2Fmkt-election%3Ftab%3Dorder",
  );
  assert.equal(
    orderRejectionRedirectPath(
      new MarketOrderApiError(
        403,
        "ERROR_CODE_KYC_TIER_INSUFFICIENT",
        "kyc required",
        "/kyc/tier-1",
      ),
      "/markets/mkt-election",
    ),
    "/kyc/tier-1?return_to=%2Fmarkets%2Fmkt-election",
  );
  assert.equal(
    orderRejectionRedirectPath(
      new MarketOrderApiError(
        402,
        "ERROR_CODE_INSUFFICIENT_FUNDS",
        "topup required",
        "/topup",
      ),
      "/markets/mkt-election",
    ),
    "/topup?return_to=%2Fmarkets%2Fmkt-election",
  );
  assert.equal(
    orderRejectionRedirectPath(
      new Error("self trade prevented"),
      "/markets/mkt-election",
    ),
    null,
  );
});
