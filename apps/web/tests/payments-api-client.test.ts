import assert from "node:assert/strict";
import { test } from "node:test";

import { fetchPaymentsRealMoneyStatus } from "../lib/payments-api.js";

test("fetchPaymentsRealMoneyStatus reads api-gateway public flag endpoint", async () => {
  const calls: string[] = [];
  const status = await fetchPaymentsRealMoneyStatus({
    baseUrl: "http://gateway.local",
    fetcher: async (input: string | URL): Promise<Response> => {
      calls.push(String(input));
      return Response.json({ real_money_enabled: true });
    },
  });

  assert.equal(calls[0], "http://gateway.local/v1/payments/real-money-status");
  assert.deepEqual(status, {
    realMoneyEnabled: true,
    source: "api-gateway",
  });
});

test("fetchPaymentsRealMoneyStatus fails closed to disabled real money", async () => {
  const status = await fetchPaymentsRealMoneyStatus({
    baseUrl: "http://gateway.local",
    fetcher: async () => {
      throw new Error("gateway unavailable");
    },
  });

  assert.deepEqual(status, {
    realMoneyEnabled: false,
    source: "fallback:api-gateway-unavailable",
  });
});
