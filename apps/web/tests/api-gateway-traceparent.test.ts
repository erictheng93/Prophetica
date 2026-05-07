import assert from "node:assert/strict";
import { test } from "node:test";

import {
  fetchWalletOverview,
  type WalletClientFactory,
} from "../lib/api-gateway.js";
import { parseTraceparent } from "../lib/observability/trace.js";

// Verifies SPEC-007 §5.1: the default fetcher used by api-gateway.ts injects
// a W3C traceparent header so api-gateway can adopt it as the trace root.

test("default fetcher attaches traceparent header on outgoing requests", async () => {
  const captured: Array<Headers> = [];
  const intercepting: WalletClientFactory = ({ baseUrl, auth, fetcher = fetch }) => ({
    async getBalance() {
      const res = await fetcher(`${baseUrl}/v1/me/wallet/balance`, {
        cache: "no-store",
        headers: { accept: "application/json", "x-user-id": auth?.userId ?? "" },
      });
      const balance = (await res.json()) as {
        user_id: string;
        available_minor_units: number;
        locked_minor_units: number;
        frozen_minor_units: number;
      };
      return {
        userId: balance.user_id,
        availableMinorUnits: BigInt(balance.available_minor_units),
        lockedMinorUnits: BigInt(balance.locked_minor_units),
        frozenMinorUnits: BigInt(balance.frozen_minor_units),
      };
    },
    async listEntries() {
      return [];
    },
    async postTransaction() {
      throw new Error("unused");
    },
  });

  const realFetch = globalThis.fetch;
  globalThis.fetch = (async (input: string | URL, init?: RequestInit): Promise<Response> => {
    captured.push(new Headers(init?.headers));
    if (String(input).includes("/v1/me/wallet/balance")) {
      return Response.json({
        user_id: "user_99",
        available_minor_units: 0,
        locked_minor_units: 0,
        frozen_minor_units: 0,
      });
    }
    return Response.json({ entries: [] });
  }) as typeof fetch;

  try {
    await fetchWalletOverview({
      baseUrl: "http://api.local",
      userId: "user_99",
      walletClientFactory: intercepting,
    });
  } finally {
    globalThis.fetch = realFetch;
  }

  assert.ok(captured.length >= 1);
  const first = captured[0];
  assert.ok(first);
  const tp = first.get("traceparent");
  assert.ok(tp, "expected traceparent header from default fetcher");
  assert.ok(parseTraceparent(tp), "header must be W3C-valid");
});
