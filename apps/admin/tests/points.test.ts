import assert from "node:assert/strict";
import { test } from "node:test";
import {
  formatMinorUnitsAsPoints,
  parsePointAmountToMinorUnits,
} from "../lib/points.js";
import {
  proxyManualPointsAdjustment,
  proxyPointsBalanceRequest,
} from "../lib/points-bff.js";

const adminViewer = {
  sub: "U-ADMIN-001",
  role: "admin" as const,
  iat: 1,
  exp: 2,
};
const affiliateViewer = {
  sub: "U-AFF-001",
  role: "affiliate" as const,
  affiliateId: "AFF-001",
  iat: 1,
  exp: 2,
};
const managerViewer = {
  sub: "U-MGR-001",
  role: "manager" as const,
  iat: 1,
  exp: 2,
};

test("parsePointAmountToMinorUnits accepts positive amounts with two decimals", () => {
  assert.equal(parsePointAmountToMinorUnits("1"), 100);
  assert.equal(parsePointAmountToMinorUnits("1.2"), 120);
  assert.equal(parsePointAmountToMinorUnits("1.23"), 123);
  assert.equal(parsePointAmountToMinorUnits("001.05"), 105);
  assert.throws(() => parsePointAmountToMinorUnits("0"));
  assert.throws(() => parsePointAmountToMinorUnits("1.234"));
  assert.throws(() => parsePointAmountToMinorUnits("-1"));
});

test("formatMinorUnitsAsPoints normalizes liability wallet balances", () => {
  assert.equal(
    formatMinorUnitsAsPoints(-12_345, { liabilityBalance: true }),
    "123.45 點",
  );
  assert.equal(formatMinorUnitsAsPoints(12_345), "123.45 點");
});

test("proxyPointsBalanceRequest forwards audited admin balance reads", async () => {
  const calls: Array<[RequestInfo | URL, RequestInit | undefined]> = [];
  const response = await proxyPointsBalanceRequest({
    viewer: adminViewer,
    userId: "user_123",
    ledgerUrl: "http://ledger.local",
    fetchImpl: async (input, init) => {
      calls.push([input, init]);
      return Response.json({
        user_id: "user_123",
        available_minor_units: -100,
        locked_minor_units: 0,
        frozen_minor_units: 0,
      });
    },
  });

  assert.equal(response.status, 200);
  assert.equal(
    String(calls[0]?.[0]),
    "http://ledger.local/internal/ledger/wallets/user_123/balance",
  );
  const headers = calls[0]?.[1]?.headers as Record<string, string>;
  assert.equal(headers["x-actor-id"], "U-ADMIN-001");
  assert.equal(headers["x-operator-role"], "admin");
  assert.match(headers["x-request-id"], /^admin-points:/);
});

test("proxyPointsBalanceRequest rejects affiliate balance reads before upstream", async () => {
  let called = false;
  const response = await proxyPointsBalanceRequest({
    viewer: affiliateViewer,
    userId: "user_123",
    ledgerUrl: "http://ledger.local",
    fetchImpl: async () => {
      called = true;
      return Response.json({});
    },
  });

  assert.equal(response.status, 403);
  assert.equal(called, false);
});

test("proxyManualPointsAdjustment overwrites actor fields and forwards idempotency", async () => {
  const calls: Array<[RequestInfo | URL, RequestInit | undefined]> = [];
  const response = await proxyManualPointsAdjustment({
    viewer: adminViewer,
    ledgerUrl: "http://ledger.local",
    request: {
      actor_user_id: "forged",
      actor_role: "affiliate",
      target_user_id: "user_123",
      direction: "grant",
      delta_minor_units: 2500,
      reason_code: "ops_adjustment",
      reason_text: "service incident credit",
      idempotency_key: "manual-points:grant:test",
      correlation_id: "ticket-123",
    },
    fetchImpl: async (input, init) => {
      calls.push([input, init]);
      return Response.json({
        txn_id: 42,
        idempotency_key: "manual-points:grant:test",
        replayed: false,
        audit_log_id: "audit:manual-points:grant:test",
      });
    },
  });

  assert.equal(response.status, 200);
  assert.equal(
    String(calls[0]?.[0]),
    "http://ledger.local/internal/ledger/manual-points-adjustments",
  );
  const init = calls[0]?.[1];
  const headers = init?.headers as Record<string, string>;
  assert.equal(headers["idempotency-key"], "manual-points:grant:test");
  assert.equal(headers["x-correlation-id"], "ticket-123");
  assert.equal(headers["x-actor-id"], "U-ADMIN-001");

  const body = JSON.parse(String(init?.body)) as Record<string, unknown>;
  assert.equal(body.actor_user_id, "U-ADMIN-001");
  assert.equal(body.actor_role, "admin");
  assert.equal(body.target_user_id, "user_123");
  assert.equal(body.delta_minor_units, 2500);
});

test("points proxy rejects manager direct adjustments before upstream", async () => {
  let called = false;
  const response = await proxyManualPointsAdjustment({
    viewer: managerViewer,
    request: {
      target_user_id: "user_123",
      direction: "deduct",
      delta_minor_units: 100,
      reason_code: "ops_adjustment",
      reason_text: "manager should propose instead",
    },
    fetchImpl: async () => {
      called = true;
      return Response.json({});
    },
  });

  assert.equal(response.status, 403);
  assert.equal(called, false);
});

test("points proxy rejects affiliate grants before upstream", async () => {
  let called = false;
  const response = await proxyManualPointsAdjustment({
    viewer: affiliateViewer,
    request: {
      target_user_id: "user_123",
      direction: "grant",
      delta_minor_units: 100,
      reason_code: "campaign",
      reason_text: "affiliate should not write ledger directly",
    },
    fetchImpl: async () => {
      called = true;
      return Response.json({});
    },
  });

  assert.equal(response.status, 403);
  assert.equal(called, false);
});
