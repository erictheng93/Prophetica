import assert from "node:assert/strict";
import { test } from "node:test";
import {
  proxyMemberActionRequest,
  proxyMemberActionReviewRequest,
  proxyMemberCreateRequest,
  proxyMemberListRequest,
} from "../lib/member-management-bff.js";

const OLD_ENV = { ...process.env };
const SECRET = "shared-secret-32-bytes-1234567890";

test.afterEach(() => {
  process.env = { ...OLD_ENV };
});

test("member list BFF forwards affiliate scope with signed admin context", async () => {
  process.env.API_GATEWAY_URL = "http://gateway.local";
  process.env.ADMIN_INTERNAL_CALL_SECRET = SECRET;
  const calls: Array<[RequestInfo | URL, RequestInit | undefined]> = [];
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async (input, init) => {
    calls.push([input, init]);
    return new Response(
      JSON.stringify({
        members: [],
        source: "user-service:member-store",
      }),
      { status: 200, headers: { "content-type": "application/json" } },
    );
  }) as typeof fetch;

  try {
    const response = await proxyMemberListRequest({
      viewer: {
        sub: "U-AFF-001",
        role: "affiliate",
        affiliateId: "AFF-001",
        iat: 1,
        exp: 2,
      },
    });

    assert.equal(response.status, 200);
    assert.equal(
      String(calls[0]?.[0]),
      "http://gateway.local/api/internal/admin/members?affiliate_id=AFF-001",
    );
    const headers = calls[0]?.[1]?.headers as Record<string, string>;
    assert.equal(headers["x-actor-id"], "U-AFF-001");
    assert.equal(headers["x-operator-role"], "affiliate");
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("member create BFF overwrites affiliate source before forwarding", async () => {
  process.env.API_GATEWAY_URL = "http://gateway.local";
  process.env.ADMIN_INTERNAL_CALL_SECRET = SECRET;
  const forwardedBodies: Record<string, unknown>[] = [];
  const calls: string[] = [];
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async (input, init) => {
    calls.push(String(input));
    forwardedBodies.push(
      JSON.parse(String(init?.body)) as Record<string, unknown>,
    );
    const isCapture = String(input).endsWith("/api/internal/referrals/capture");
    return new Response(JSON.stringify({ member: { user_id: "MEM-BFF" } }), {
      status: isCapture ? 200 : 201,
      headers: { "content-type": "application/json" },
    });
  }) as typeof fetch;

  try {
    const response = await proxyMemberCreateRequest({
      viewer: {
        sub: "U-AFF-001",
        role: "affiliate",
        affiliateId: "AFF-001",
        iat: 1,
        exp: 2,
      },
      request: {
        user_id: "MEM-BFF",
        display_name: "BFF Member",
        email: "bff@example.com",
        phone: "0912345000",
        country: "TW",
        timezone: "Asia/Taipei",
        tier: "Standard",
        affiliate_id: "AFF-OTHER",
      },
    });

    assert.equal(response.status, 201);
    assert.equal(calls[0], "http://gateway.local/api/internal/admin/members");
    assert.equal(
      calls[1],
      "http://gateway.local/api/internal/referrals/capture",
    );
    assert.equal(forwardedBodies[0]?.affiliate_id, "AFF-001");
    assert.deepEqual(forwardedBodies[0]?.registration_audit, {
      source: "apps-admin:member-create-bff",
      referral_capture_required: true,
      referral_code: "AFF-001",
      referred_user_id: "MEM-BFF",
      actor_user_id: "U-AFF-001",
      actor_role: "affiliate",
    });
    assert.deepEqual(forwardedBodies[1], {
      referral_code: "AFF-001",
      referred_user_id: "MEM-BFF",
    });
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("member create BFF reports referral capture failure after attributed create", async () => {
  process.env.API_GATEWAY_URL = "http://gateway.local";
  process.env.ADMIN_INTERNAL_CALL_SECRET = SECRET;
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async (input) => {
    if (String(input).endsWith("/api/internal/referrals/capture")) {
      return new Response(
        JSON.stringify({ error: "invalid_affiliate_request" }),
        {
          status: 400,
          headers: { "content-type": "application/json" },
        },
      );
    }
    return new Response(JSON.stringify({ member: { user_id: "MEM-BFF" } }), {
      status: 201,
      headers: { "content-type": "application/json" },
    });
  }) as typeof fetch;

  try {
    const response = await proxyMemberCreateRequest({
      viewer: {
        sub: "U-AFF-001",
        role: "affiliate",
        affiliateId: "AFF-001",
        iat: 1,
        exp: 2,
      },
      request: {
        user_id: "MEM-BFF",
        display_name: "BFF Member",
        email: "bff@example.com",
        phone: "0912345000",
        country: "TW",
        timezone: "Asia/Taipei",
        tier: "Standard",
        affiliate_id: "AFF-OTHER",
      },
    });

    assert.equal(response.status, 400);
    assert.deepEqual(await response.json(), {
      error: "referral_capture_failed",
    });
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("member action BFF validates target through gateway before forwarding action", async () => {
  process.env.API_GATEWAY_URL = "http://gateway.local";
  process.env.ADMIN_INTERNAL_CALL_SECRET = SECRET;
  const calls: Array<[RequestInfo | URL, RequestInit | undefined]> = [];
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async (input, init) => {
    calls.push([input, init]);
    if (
      String(input).endsWith("/api/internal/admin/members/MEM-1042/summary")
    ) {
      return new Response(
        JSON.stringify({
          member: { user_id: "MEM-1042" },
          source: { affiliate_id: "AFF-014" },
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    }
    return new Response(
      JSON.stringify({
        action_id: "mem-act-000123",
        target_user_id: "MEM-1042",
        action_type: "restrict_withdrawals",
        status: "pending_second_approval",
        requires_second_approval: true,
        audit: {
          actor_user_id: "U-MGR-001",
          actor_role: "manager",
          target_user_id: "MEM-1042",
          ticket_id: "OPS-1",
          correlation_id: "corr-1",
          ledger_transaction_posted: false,
        },
      }),
      { status: 202, headers: { "content-type": "application/json" } },
    );
  }) as typeof fetch;

  try {
    const response = await proxyMemberActionRequest({
      viewer: { sub: "U-MGR-001", role: "manager", iat: 1, exp: 2 },
      request: {
        target_user_id: "MEM-1042",
        action_type: "restrict_withdrawals",
        ticket_id: "OPS-1",
        reason: "risk hold",
      },
    });

    assert.equal(response.status, 202);
    assert.equal(calls.length, 2);
    assert.equal(
      String(calls[0]?.[0]),
      "http://gateway.local/api/internal/admin/members/MEM-1042/summary",
    );
    assert.equal(
      String(calls[1]?.[0]),
      "http://gateway.local/api/internal/admin/members/actions",
    );
    assert.equal(calls[1]?.[1]?.method, "POST");
  } finally {
    globalThis.fetch = originalFetch;
  }
});

test("member action review BFF forwards review decision to gateway", async () => {
  process.env.API_GATEWAY_URL = "http://gateway.local";
  process.env.ADMIN_INTERNAL_CALL_SECRET = SECRET;
  let forwardedBody: Record<string, unknown> | null = null;
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async (input, init) => {
    forwardedBody = JSON.parse(String(init?.body)) as Record<string, unknown>;
    assert.equal(
      String(input),
      "http://gateway.local/api/internal/admin/members/actions/mem-act-000123/reviews",
    );
    return new Response(JSON.stringify({ status: "approved" }), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  }) as typeof fetch;

  try {
    const response = await proxyMemberActionReviewRequest({
      viewer: { sub: "U-ADMIN-001", role: "admin", iat: 1, exp: 2 },
      actionId: "mem-act-000123",
      request: {
        decision: "approve",
        reason: "confirmed",
      },
    });

    assert.equal(response.status, 200);
    assert.deepEqual(forwardedBody, {
      decision: "approve",
      reason: "confirmed",
    });
  } finally {
    globalThis.fetch = originalFetch;
  }
});
