import assert from "node:assert/strict";
import { test } from "node:test";

import { fetchAffiliateDashboard } from "../lib/affiliate-api.js";

const apiDashboard = {
  user_id: "user_01",
  referral_code: "a3f9c12be801_X9PK",
  share_url: "https://proph.net/?ref=a3f9c12be801_X9PK",
  summary: {
    successful_referrals_count: 2,
    signup_bonus_minor_units: 20_000,
    trade_commission_minor_units: 22_000,
    total_rewards_minor_units: 42_000,
    last_30d_commission_minor_units: 5_000,
    last_12m_commission_minor_units: 22_000,
  },
  referrals: [
    {
      referred_id: "user_42",
      masked_username: "ali*e",
      referred_at_day: 19_843,
      status: "qualified",
      cumulative_commission_minor_units: 12_000,
    },
    {
      referred_id: "user_77",
      masked_username: "bob*o",
      referred_at_day: 19_860,
      status: "pending",
      cumulative_commission_minor_units: 0,
    },
  ],
};

test("fetchAffiliateDashboard returns a safe dashboard without bearer auth", async () => {
  let called = false;
  const dashboard = await fetchAffiliateDashboard({
    baseUrl: "http://gateway.local",
    userId: "user_01",
    limit: 10,
    fetcher: async (): Promise<Response> => {
      called = true;
      return Response.json(apiDashboard);
    },
  });

  assert.equal(called, false);
  assert.equal(dashboard.source, "fallback:api-gateway-unavailable");
  assert.equal(dashboard.userId, "user_01");
});

test("fetchAffiliateDashboard hits api-gateway with bearer auth and maps minor units to points", async () => {
  const calls: {
    url: string;
    authorization: string | undefined;
  }[] = [];

  const dashboard = await fetchAffiliateDashboard({
    baseUrl: "http://gateway.local",
    userId: "user_01",
    bearerToken: "jwt-user-service",
    limit: 10,
    fetcher: async (input, init): Promise<Response> => {
      const headers = new Headers(init?.headers);
      calls.push({
        url: String(input),
        authorization: headers.get("authorization") ?? undefined,
      });
      return Response.json(apiDashboard);
    },
  });

  assert.equal(
    calls[0]?.url,
    "http://gateway.local/v1/account/affiliate?limit=10",
  );
  assert.equal(calls[0]?.authorization, "Bearer jwt-user-service");
  assert.equal(dashboard.source, "api-gateway");
  assert.equal(dashboard.referralCode, "a3f9c12be801_X9PK");
  assert.equal(dashboard.shareUrl, "https://proph.net/?ref=a3f9c12be801_X9PK");
  assert.equal(dashboard.summary.successfulReferralsCount, 2);
  assert.equal(dashboard.summary.totalRewardsPoints, 420);
  assert.equal(dashboard.summary.signupBonusPoints, 200);
  assert.equal(dashboard.summary.tradeCommissionPoints, 220);
  assert.equal(dashboard.summary.last30dCommissionPoints, 50);
  assert.equal(dashboard.summary.last12mCommissionPoints, 220);
  assert.equal(dashboard.referrals.length, 2);
  assert.equal(dashboard.referrals[0]?.maskedUsername, "ali*e");
  assert.equal(dashboard.referrals[0]?.cumulativeCommissionPoints, 120);
  assert.equal(dashboard.referrals[0]?.status, "qualified");
  assert.equal(dashboard.referrals[1]?.status, "pending");
});

test("fetchAffiliateDashboard does not call api-gateway without bearer token by default", async () => {
  let called = false;
  const dashboard = await fetchAffiliateDashboard({
    baseUrl: "http://gateway.local",
    userId: "user_01",
    fetcher: async (): Promise<Response> => {
      called = true;
      return Response.json(apiDashboard);
    },
  });

  assert.equal(called, false);
  assert.equal(dashboard.source, "fallback:api-gateway-unavailable");
  assert.equal(dashboard.userId, "user_01");
});

test("fetchAffiliateDashboard forwards user-service bearer token when available", async () => {
  const calls: Array<{ headers: Record<string, string> }> = [];
  await fetchAffiliateDashboard({
    baseUrl: "http://gateway.local",
    userId: "user_01",
    bearerToken: "jwt-user-service",
    fetcher: async (_input, init): Promise<Response> => {
      calls.push({
        headers: Object.fromEntries(new Headers(init?.headers).entries()),
      });
      return Response.json(apiDashboard);
    },
  });

  assert.equal(calls[0]?.headers.authorization, "Bearer jwt-user-service");
  assert.equal(calls[0]?.headers["x-user-id"], undefined);
});

test("fetchAffiliateDashboard returns empty dashboard when api-gateway is unavailable", async () => {
  const dashboard = await fetchAffiliateDashboard({
    baseUrl: "http://gateway.local",
    userId: "user_01",
    bearerToken: "jwt-user-service",
    fetcher: async () => {
      throw new Error("connection refused");
    },
  });

  assert.equal(dashboard.source, "fallback:api-gateway-unavailable");
  assert.equal(dashboard.referralCode, "");
  assert.equal(dashboard.summary.totalRewardsPoints, 0);
  assert.equal(dashboard.referrals.length, 0);
});

test("fetchAffiliateDashboard returns empty dashboard when affiliate data is not found", async () => {
  const dashboard = await fetchAffiliateDashboard({
    baseUrl: "http://gateway.local",
    userId: "user_99",
    bearerToken: "jwt-user-service",
    fetcher: async () =>
      new Response(
        JSON.stringify({ error: "affiliate_not_found", message: "no data" }),
        { status: 404, headers: { "content-type": "application/json" } },
      ),
  });

  assert.equal(dashboard.source, "fallback:affiliate-not-found");
  assert.equal(dashboard.referralCode, "");
});

test("fetchAffiliateDashboard normalizes unknown statuses to pending", async () => {
  const dashboard = await fetchAffiliateDashboard({
    baseUrl: "http://gateway.local",
    bearerToken: "jwt-user-service",
    fetcher: async () =>
      Response.json({
        ...apiDashboard,
        referrals: [
          {
            ...apiDashboard.referrals[0],
            status: "garbage_status_from_future",
          },
        ],
      }),
  });

  assert.equal(dashboard.referrals[0]?.status, "pending");
});
