import assert from "node:assert/strict";
import { test } from "node:test";
import { NextRequest } from "next/server.js";
import { middleware, proxy } from "../middleware.js";
import { signSession } from "../lib/session-crypto.js";

const SECRET = "test-secret-do-not-use-in-prod";
process.env.ADMIN_SESSION_SECRET = SECRET;

test("middleware exports proxy alias and default export", () => {
  assert.equal(typeof middleware, "function");
  assert.equal(proxy, middleware);
});

test("middleware allows public routes without authentication", async () => {
  const publicPaths = [
    "/login",
    "/api/health",
    "/api/auth/session",
    "/robots.txt",
    "/favicon.ico",
  ];
  for (const path of publicPaths) {
    const req = new NextRequest(`http://localhost:3000${path}`);
    const res = await middleware(req);
    assert.equal(res.status, 200);
    assert.equal(res.headers.get("location"), null);
  }
});

test("middleware redirects unauthenticated request to /login with from query param", async () => {
  const req = new NextRequest("http://localhost:3000/members");
  const res = await middleware(req);
  assert.equal(res.status, 307);
  const location = res.headers.get("location");
  assert.ok(location);
  assert.match(location, /\/login\?from=%2Fmembers/);
});

test("middleware redirects root path to /login without from query param", async () => {
  const req = new NextRequest("http://localhost:3000/");
  const res = await middleware(req);
  assert.equal(res.status, 307);
  const location = res.headers.get("location");
  assert.ok(location);
  assert.match(location, /\/login$/);
});

test("middleware enforces Cloudflare Access in production for internal paths", async () => {
  const originalEnv = process.env.NODE_ENV;
  try {
    (process.env as Record<string, string | undefined>).NODE_ENV = "production";
    const req = new NextRequest("http://localhost:3000/members");
    const res = await middleware(req);
    assert.equal(res.status, 403);
  } finally {
    (process.env as Record<string, string | undefined>).NODE_ENV = originalEnv;
  }
});

test("middleware enforces RBAC for authenticated sessions", async () => {
  const affiliateToken = await signSession(
    { sub: "AFF-1", role: "affiliate", affiliateId: "AFF-1" },
    SECRET,
  );

  // Affiliate accessing points route is redirected to affiliate landing
  const req = new NextRequest("http://localhost:3000/points", {
    headers: {
      cookie: `proph_admin_session=${affiliateToken}`,
    },
  });
  const res = await middleware(req);
  assert.equal(res.status, 307);
  assert.match(res.headers.get("location") || "", /\/me\/affiliate/);

  // Manager accessing admin-only settings route is redirected to /403
  const managerToken = await signSession(
    { sub: "MGR-1", role: "manager" },
    SECRET,
  );
  const managerReq = new NextRequest("http://localhost:3000/settings/accounts", {
    headers: {
      cookie: `proph_admin_session=${managerToken}`,
    },
  });
  const managerRes = await middleware(managerReq);
  assert.equal(managerRes.status, 307);
  assert.match(managerRes.headers.get("location") || "", /\/403/);

  // Admin accessing admin-only settings route is allowed
  const adminToken = await signSession(
    { sub: "ADM-1", role: "admin" },
    SECRET,
  );
  const adminReq = new NextRequest("http://localhost:3000/settings/accounts", {
    headers: {
      cookie: `proph_admin_session=${adminToken}`,
    },
  });
  const adminRes = await middleware(adminReq);
  assert.equal(adminRes.status, 200);
  assert.equal(adminRes.headers.get("location"), null);
});
