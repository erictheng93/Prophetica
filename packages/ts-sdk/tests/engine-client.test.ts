import assert from "node:assert/strict";
import { test } from "node:test";

import { ApiError, createEngineClient } from "../src/index.js";

test("engine client places orders through matching-engine REST facade", async () => {
  const calls: Array<{
    url: string;
    method?: string;
    headers: Record<string, string>;
    body: unknown;
  }> = [];
  const client = createEngineClient({
    baseUrl: "http://engine.local/",
    auth: { userId: "42" },
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

  const response = await client.placeOrder({
    marketId: "mkt-election",
    side: "buy",
    outcome: "YES",
    type: "LIMIT",
    pricePts: 55,
    quantity: 3,
    timeInForce: "GTC",
    clientOrderId: "client-01",
  });

  assert.equal(
    calls[0]?.url,
    "http://engine.local/api/v1/markets/mkt-election/orders",
  );
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
  assert.deepEqual(response, {
    orderId: "ord_01HZ0000000000000000000000",
    status: "ACTIVE",
    filledQty: 0,
    avgPricePts: 0,
    remainingQty: 3,
    feePts: 0,
  });
});

test("engine client surfaces matching-engine error bodies", async () => {
  const client = createEngineClient({
    baseUrl: "http://engine.local",
    fetcher: async () =>
      Response.json({ error: "self trade prevented" }, { status: 412 }),
  });

  await assert.rejects(
    () =>
      client.placeOrder({
        marketId: "mkt-election",
        side: "buy",
        outcome: "YES",
        type: "LIMIT",
        pricePts: 55,
        quantity: 3,
        timeInForce: "GTC",
      }),
    (error: unknown) => {
      assert.ok(error instanceof ApiError);
      assert.equal(error.status, 412);
      assert.equal(error.message, "self trade prevented");
      return true;
    },
  );
});
