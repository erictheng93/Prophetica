import assert from "node:assert/strict";
import { test } from "node:test";
import {
  archiveAdminAffiliate,
  createAdminAffiliate,
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
      id: "AFF-200",
      userId: "user_aff_200",
      displayName: "Orion Partners",
      emailMasked: "o***@example.com",
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

  assert.equal(String(calls[0]?.[0]), "http://gateway.local/api/internal/admin/affiliates");
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
  assert.equal(headers["content-type"], "application/json");
});

test("affiliate BFF rejects affiliate role mutations before upstream call", async () => {
  let called = false;
  const result = await createAdminAffiliate(
    { sub: "U-AFF-001", role: "affiliate", affiliateId: "AFF-001", iat: 1, exp: 2 },
    {
      id: "AFF-201",
      userId: "user_aff_201",
      displayName: "Blocked",
      emailMasked: "b***@example.com",
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
