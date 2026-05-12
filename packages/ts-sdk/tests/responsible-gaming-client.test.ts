import assert from "node:assert/strict";
import { test } from "node:test";

import { createResponsibleGamingClient } from "../src/index.js";

test("responsible gaming client reads self-exclusion status with auth context", async () => {
  const calls: Array<{ url: string; headers: Record<string, string> }> = [];
  const client = createResponsibleGamingClient({
    baseUrl: "http://api.local/",
    auth: { userId: "user_42" },
    fetcher: async (input, init) => {
      calls.push({
        url: String(input),
        headers: Object.fromEntries(new Headers(init?.headers).entries()),
      });

      return Response.json({
        user_id: "user_42",
        active: true,
        record: {
          record_id: "se_user_42_1780000000",
          user_id: "user_42",
          mode: "timeout",
          period: "six_months",
          points_disposition: null,
          reason: "need a long break",
          starts_at_epoch_seconds: 1_780_000_000,
          ends_at_epoch_seconds: 1_795_811_200,
          frozen_minor_units: "12500",
        },
        source: "api-gateway",
      });
    },
  });

  const status = await client.getSelfExclusionStatus();

  assert.equal(
    calls[0]?.url,
    "http://api.local/v1/responsible-gaming/self-exclusion",
  );
  assert.equal(calls[0]?.headers["x-user-id"], "user_42");
  assert.equal(status.active, true);
  assert.equal(status.record?.frozenMinorUnits, 12_500n);
  assert.equal(status.record?.period, "six_months");
});

test("responsible gaming client submits self-exclusion with idempotency key", async () => {
  let requestBody: unknown;
  const client = createResponsibleGamingClient({
    baseUrl: "http://api.local",
    auth: { userId: "user_42" },
    fetcher: async (input, init) => {
      requestBody = JSON.parse(String(init?.body));
      const headers = new Headers(init?.headers);
      assert.equal(
        String(input),
        "http://api.local/v1/responsible-gaming/self-exclusion",
      );
      assert.equal(
        headers.get("idempotency-key"),
        "rg-self-exclusion:user_42:001",
      );
      assert.equal(headers.get("x-user-id"), "user_42");
      return Response.json({
        record: {
          record_id: "se_user_42_1780000000",
          user_id: "user_42",
          mode: "permanent",
          period: null,
          points_disposition: "donate",
          reason: null,
          starts_at_epoch_seconds: 1_780_000_000,
          ends_at_epoch_seconds: null,
          frozen_minor_units: 0,
        },
        ledger_idempotency_key:
          "self-exclusion:user_42:rg-self-exclusion:user_42:001",
        replayed: false,
      });
    },
  });

  const response = await client.submitSelfExclusion({
    mode: "permanent",
    pointsDisposition: "donate",
    consent: true,
    idempotencyKey: "rg-self-exclusion:user_42:001",
  });

  assert.deepEqual(requestBody, {
    mode: "permanent",
    points_disposition: "donate",
    consent: true,
  });
  assert.equal(response.record.mode, "permanent");
  assert.equal(response.record.pointsDisposition, "donate");
  assert.equal(response.replayed, false);
});
