import assert from "node:assert/strict";
import { test } from "node:test";
import {
  archiveAdminAffiliate,
  createAdminAffiliate,
  fetchAffiliateDashboard,
  fetchAdminAffiliateDetail,
  fetchAdminAffiliateList,
  reviewAffiliateCommissionAdjustment,
  reviewAffiliatePayout,
  updateAdminAffiliate,
} from "../lib/affiliate-management-bff.js";

const OLD_ENV = { ...process.env };

test.afterEach(() => {
  process.env = { ...OLD_ENV };
});

test("affiliate BFF forwards CRUD mutations with signed admin context", async () => {
  process.env.API_GATEWAY_URL = "http://gateway.local";
  process.env.ADMIN_INTERNAL_CALL_SECRET = "shared-secret-32-bytes-1234567890";
  const calls: Array<[RequestInfo | URL, RequestInit | undefined]> = [];
  const viewer = { sub: "U-ADMIN-001", role: "admin" as const, iat: 1, exp: 2 };
  const fetchImpl = async (input: RequestInfo | URL, init?: RequestInit) => {
    calls.push([input, init]);
    if (init?.method === "POST") {
      return new Response(
        JSON.stringify({
          idempotencyKey: "affiliate-account-test",
          status: "completed",
          targetUserId: "U-AFF-200",
          affiliateId: "AFF-200",
          actorId: "U-ADMIN-001",
          actorRole: "admin",
          lastError: null,
          affiliate: {
            affiliate: { id: "AFF-200" },
            auditCorrelationId: "audit:affiliate",
          },
        }),
        { status: 201 },
      );
    }
    return new Response(
      JSON.stringify({
        affiliate: { id: "AFF-200" },
        auditCorrelationId: "audit:affiliate",
      }),
      { status: init?.method === "POST" ? 201 : 200 },
    );
  };
  const originalFetch = globalThis.fetch;
  globalThis.fetch = fetchImpl as typeof fetch;
  try {
    await createAdminAffiliate(viewer, {
      userId: "U-AFF-200",
      affiliateId: "AFF-200",
      displayName: "Orion Partners",
      email: "ops@example.com",
      phone: "0912345678",
      password: "initial-password",
      reason: "onboarding",
      authorizationBasis: "SPEC-011",
    });
    await updateAdminAffiliate(viewer, "AFF-200", {
      commissionRateBps: 700,
      reason: "rate",
      authorizationBasis: "SPEC-011",
    });
    await archiveAdminAffiliate(viewer, "AFF-200", {
      reason: "offboarding",
      authorizationBasis: "termination",
    });
  } finally {
    globalThis.fetch = originalFetch;
  }

  assert.equal(
    String(calls[0]?.[0]),
    "http://gateway.local/api/internal/admin/affiliate-accounts",
  );
  assert.equal(calls[0]?.[1]?.method, "POST");
  assert.equal(
    String(calls[1]?.[0]),
    "http://gateway.local/api/internal/admin/affiliates/AFF-200",
  );
  assert.equal(calls[1]?.[1]?.method, "PATCH");
  assert.equal(calls[2]?.[1]?.method, "DELETE");
  const headers = calls[0]?.[1]?.headers as Record<string, string>;
  assert.equal(headers["x-actor-id"], "U-ADMIN-001");
  assert.equal(headers["x-operator-role"], "admin");
  assert.ok(headers["idempotency-key"]);
  assert.equal(headers["content-type"], "application/json");
});

test("affiliate BFF does not use mock fallback when gateway is unavailable", async () => {
  process.env.AFFILIATE_ADMIN_ALLOW_MOCK_FALLBACK = "true";
  delete process.env.API_GATEWAY_URL;
  const viewer = { sub: "U-ADMIN-001", role: "admin" as const, iat: 1, exp: 2 };

  const affiliates = await fetchAdminAffiliateList(viewer);
  const detail = await fetchAdminAffiliateDetail(viewer, "AFF-001");

  assert.deepEqual(affiliates, []);
  assert.equal(detail, null);
});

test("affiliate BFF forwards list and detail reads to api-gateway", async () => {
  process.env.API_GATEWAY_URL = "http://gateway.local";
  process.env.ADMIN_INTERNAL_CALL_SECRET = "shared-secret-32-bytes-1234567890";
  const calls: Array<[RequestInfo | URL, RequestInit | undefined]> = [];
  const viewer = { sub: "U-ADMIN-001", role: "admin" as const, iat: 1, exp: 2 };
  const fetchImpl = async (input: RequestInfo | URL, init?: RequestInit) => {
    calls.push([input, init]);
    if (String(input).endsWith("/AFF-001")) {
      return Response.json({ id: "AFF-001", displayName: "Live Affiliate" });
    }
    return Response.json([{ id: "AFF-001", displayName: "Live Affiliate" }]);
  };
  const originalFetch = globalThis.fetch;
  globalThis.fetch = fetchImpl as typeof fetch;
  try {
    const affiliates = await fetchAdminAffiliateList(viewer);
    const detail = await fetchAdminAffiliateDetail(viewer, "AFF-001");
    assert.equal(affiliates[0]?.id, "AFF-001");
    assert.equal(detail?.id, "AFF-001");
  } finally {
    globalThis.fetch = originalFetch;
  }

  assert.equal(
    String(calls[0]?.[0]),
    "http://gateway.local/api/internal/admin/affiliates",
  );
  assert.equal(
    String(calls[1]?.[0]),
    "http://gateway.local/api/internal/admin/affiliates/AFF-001",
  );
  const headers = calls[0]?.[1]?.headers as Record<string, string>;
  assert.equal(headers["x-actor-id"], "U-ADMIN-001");
  assert.equal(headers["x-operator-role"], "admin");
});

test("affiliate BFF rejects affiliate role mutations before upstream call", async () => {
  let called = false;
  const result = await createAdminAffiliate(
    {
      sub: "U-AFF-001",
      role: "affiliate",
      affiliateId: "AFF-001",
      iat: 1,
      exp: 2,
    },
    {
      userId: "U-AFF-200",
      affiliateId: "AFF-200",
      displayName: "Blocked",
      email: "blocked@example.com",
      phone: "0912345678",
      password: "initial-password",
      reason: "blocked",
      authorizationBasis: "SPEC-011",
    },
  );
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async () => {
    called = true;
    return new Response(null, { status: 200 });
  }) as typeof fetch;
  globalThis.fetch = originalFetch;

  assert.equal(result, null);
  assert.equal(called, false);
});

test("affiliate portal dashboard requires bearer auth for gateway reads", async () => {
  process.env.API_GATEWAY_URL = "http://gateway.local";
  const viewer = {
    sub: "U-AFF-001",
    role: "affiliate" as const,
    affiliateId: "AFF-001",
    iat: 1,
    exp: 2,
  };
  const calls: Array<[RequestInfo | URL, RequestInit | undefined]> = [];
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    calls.push([input, init]);
    return Response.json({
      user_id: "U-AFF-001",
      referral_code: "AFF-001",
      share_url: "https://example.test/r/AFF-001",
      summary: {
        successful_referrals_count: 0,
        signup_bonus_minor_units: 0,
        trade_commission_minor_units: 0,
        total_rewards_minor_units: 0,
        last_30d_commission_minor_units: 0,
        last_12m_commission_minor_units: 0,
      },
      referrals: [],
    });
  }) as typeof fetch;
  try {
    assert.equal(await fetchAffiliateDashboard(viewer), null);
    const dashboard = await fetchAffiliateDashboard(viewer, "jwt-user-service");
    assert.equal(dashboard?.referral_code, "AFF-001");
  } finally {
    globalThis.fetch = originalFetch;
  }

  assert.equal(calls.length, 1);
  assert.equal(
    String(calls[0]?.[0]),
    "http://gateway.local/v1/account/affiliate?limit=200",
  );
  const headers = calls[0]?.[1]?.headers as Record<string, string>;
  assert.equal(headers.authorization, "Bearer jwt-user-service");
  assert.equal(headers["x-user-id"], undefined);
});

test("affiliate BFF strips managingManagerId from PATCH payload for manager viewer", async () => {
  // SPEC-017 §6.3.5.2 / §6.3.5.4: deny-by-presence. BFF removes the field so a
  // manager's edit form (which may round-trip the read-only ownership) cannot
  // accidentally trigger a 403 at the gateway.
  process.env.API_GATEWAY_URL = "http://gateway.local";
  process.env.ADMIN_INTERNAL_CALL_SECRET = "shared-secret-32-bytes-1234567890";
  const calls: Array<[RequestInfo | URL, RequestInit | undefined]> = [];
  const fetchImpl = async (input: RequestInfo | URL, init?: RequestInit) => {
    calls.push([input, init]);
    return new Response(
      JSON.stringify({
        affiliate: { id: "AFF-OWN" },
        auditCorrelationId: "audit:affiliate",
      }),
      { status: 200 },
    );
  };
  const originalFetch = globalThis.fetch;
  globalThis.fetch = fetchImpl as typeof fetch;
  try {
    const managerViewer = {
      sub: "U-MGR-001",
      role: "manager" as const,
      iat: 1,
      exp: 2,
    };
    await updateAdminAffiliate(managerViewer, "AFF-OWN", {
      commissionRateBps: 650,
      managingManagerId: "U-MGR-001",
      reason: "rate adjustment",
      authorizationBasis: "SPEC-017 §6.3.5",
    });

    const adminViewer = {
      sub: "U-ADMIN-001",
      role: "admin" as const,
      iat: 1,
      exp: 2,
    };
    await updateAdminAffiliate(adminViewer, "AFF-OWN", {
      managingManagerId: "U-MGR-002",
      reason: "reassign",
      authorizationBasis: "SPEC-017 §6.3.5.2",
    });
  } finally {
    globalThis.fetch = originalFetch;
  }

  const managerBody = JSON.parse(String(calls[0]?.[1]?.body)) as Record<
    string,
    unknown
  >;
  assert.ok(
    !("managingManagerId" in managerBody),
    "manager payload must not contain managingManagerId",
  );
  assert.equal(managerBody.commissionRateBps, 650);

  const adminBody = JSON.parse(String(calls[1]?.[1]?.body)) as Record<
    string,
    unknown
  >;
  assert.equal(
    adminBody.managingManagerId,
    "U-MGR-002",
    "admin payload retains managingManagerId for reassignment",
  );
});

test("affiliate BFF forwards review approve and reject decisions with signed admin context", async () => {
  process.env.API_GATEWAY_URL = "http://gateway.local";
  process.env.ADMIN_INTERNAL_CALL_SECRET = "shared-secret-32-bytes-1234567890";
  const calls: Array<[RequestInfo | URL, RequestInit | undefined]> = [];
  const viewer = { sub: "U-ADMIN-001", role: "admin" as const, iat: 1, exp: 2 };
  const fetchImpl = async (input: RequestInfo | URL, init?: RequestInit) => {
    calls.push([input, init]);
    return new Response(
      JSON.stringify({
        review: { id: "review-1", status: "approved" },
        auditCorrelationId: "audit:review-1",
      }),
      { status: 200 },
    );
  };
  const originalFetch = globalThis.fetch;
  globalThis.fetch = fetchImpl as typeof fetch;
  try {
    await reviewAffiliateCommissionAdjustment(viewer, "COM-1", {
      decision: "approved",
      reason: "invoice reconciled",
    });
    await reviewAffiliatePayout(viewer, "PAY-1", {
      decision: "rejected",
      reason: "bank verification failed",
    });
  } finally {
    globalThis.fetch = originalFetch;
  }

  assert.equal(
    String(calls[0]?.[0]),
    "http://gateway.local/api/internal/admin/affiliate-commission-adjustments/COM-1/review",
  );
  assert.equal(calls[0]?.[1]?.method, "POST");
  assert.deepEqual(JSON.parse(String(calls[0]?.[1]?.body)), {
    decision: "approved",
    reason: "invoice reconciled",
  });
  assert.equal(
    String(calls[1]?.[0]),
    "http://gateway.local/api/internal/admin/affiliate-payouts/PAY-1/review",
  );
  assert.deepEqual(JSON.parse(String(calls[1]?.[1]?.body)), {
    decision: "rejected",
    reason: "bank verification failed",
  });
  const headers = calls[0]?.[1]?.headers as Record<string, string>;
  assert.equal(headers["x-actor-id"], "U-ADMIN-001");
  assert.equal(headers["x-operator-role"], "admin");
  assert.equal(headers["content-type"], "application/json");
});
