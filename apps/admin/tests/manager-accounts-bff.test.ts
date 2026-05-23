import assert from "node:assert/strict";
import { test } from "node:test";
import {
  createManagerAccount,
  disableManagerAccount,
  fetchManagerAccounts,
  updateManagerAccount,
} from "../lib/manager-accounts-bff.js";

const OLD_ENV = { ...process.env };

test.afterEach(() => {
  process.env = { ...OLD_ENV };
});

test("manager accounts BFF forwards signed CRUD requests", async () => {
  process.env.API_GATEWAY_URL = "http://gateway.local";
  process.env.ADMIN_INTERNAL_CALL_SECRET = "shared-secret-32-bytes-1234567890";
  const calls: Array<[RequestInfo | URL, RequestInit | undefined]> = [];
  const viewer = { sub: "U-ADMIN-001", role: "admin" as const, iat: 1, exp: 2 };
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    calls.push([input, init]);
    return new Response(
      JSON.stringify({
        accounts: [
          {
            user_id: "MAN-001",
            email: "manager@example.com",
            phone: "0912345678",
            role: "manager",
            affiliate_id: null,
            disabled: false,
          },
        ],
        source: "user-service:role-accounts",
      }),
      { status: 200 },
    );
  }) as typeof fetch;
  try {
    await fetchManagerAccounts(viewer);
    await createManagerAccount(viewer, {
      userId: "MAN-002",
      displayName: "Manager Two",
      email: "manager2@example.com",
      phone: "0912345679",
      password: "correct horse manager",
      reason: "hire",
      authorizationBasis: "ticket-1",
    });
    await updateManagerAccount(viewer, "MAN-002", {
      email: "manager2-new@example.com",
      phone: "0912345680",
      reason: "contact update",
      authorizationBasis: "ticket-2",
    });
    await disableManagerAccount(viewer, "MAN-002");
  } finally {
    globalThis.fetch = originalFetch;
  }

  assert.equal(
    String(calls[0]?.[0]),
    "http://gateway.local/api/internal/admin/role-accounts?role=manager",
  );
  assert.equal(calls[0]?.[1]?.method, "GET");
  assert.equal(
    String(calls[1]?.[0]),
    "http://gateway.local/api/internal/admin/members",
  );
  assert.equal(calls[1]?.[1]?.method, "POST");
  assert.equal(
    String(calls[2]?.[0]),
    "http://gateway.local/api/internal/admin/members/MAN-002",
  );
  assert.equal(calls[2]?.[1]?.method, "PATCH");
  assert.equal(calls[3]?.[1]?.method, "DELETE");
  const createBody = JSON.parse(String(calls[1]?.[1]?.body));
  assert.equal(createBody.role, "manager");
  assert.equal(createBody.authorization_basis, "ticket-1");
  const headers = calls[1]?.[1]?.headers as Record<string, string>;
  assert.equal(headers["x-actor-id"], "U-ADMIN-001");
  assert.equal(headers["x-operator-role"], "admin");
  assert.equal(headers["content-type"], "application/json");
});

test("manager accounts BFF rejects non-admin mutations before upstream call", async () => {
  let called = false;
  const originalFetch = globalThis.fetch;
  globalThis.fetch = (async () => {
    called = true;
    return new Response(null, { status: 200 });
  }) as typeof fetch;
  try {
    const viewer = {
      sub: "MAN-001",
      role: "manager" as const,
      iat: 1,
      exp: 2,
    };
    assert.equal(
      await createManagerAccount(viewer, {
        userId: "MAN-002",
        displayName: "Manager Two",
        email: "manager2@example.com",
        phone: "0912345679",
        password: "correct horse manager",
        reason: "blocked",
        authorizationBasis: "ticket-1",
      }),
      null,
    );
    assert.equal(await updateManagerAccount(viewer, "MAN-002", {
      reason: "blocked",
      authorizationBasis: "ticket-2",
    }), null);
    assert.equal(await disableManagerAccount(viewer, "MAN-002"), null);
  } finally {
    globalThis.fetch = originalFetch;
  }
  assert.equal(called, false);
});
