import assert from "node:assert/strict";
import { createHash, createHmac } from "node:crypto";
import { test } from "node:test";
import { proxyAnalyticsRequest } from "../lib/analytics-bff.js";

const SECRET = "shared-secret-32-bytes-1234567890";

test("proxyAnalyticsRequest signs and forwards admin requests to api-gateway", async () => {
  const calls: Array<[RequestInfo | URL, RequestInit | undefined]> = [];
  const response = await proxyAnalyticsRequest({
    viewer: { sub: "U-ADMIN-001", role: "admin", iat: 1, exp: 2 },
    method: "POST",
    adminPath: "/api/internal/admin/report-jobs",
    body: '{"templateCode":"ops-kpi-daily"}',
    gatewayUrl: "http://gateway.local",
    secret: SECRET,
    nowSeconds: 1_780_000_000,
    fetchImpl: async (input, init) => {
      calls.push([input, init]);
      return new Response(JSON.stringify({ ok: true }), { status: 200 });
    },
  });

  assert.equal(response.status, 200);
  assert.equal(String(calls[0]?.[0]), "http://gateway.local/api/internal/admin/report-jobs");
  const headers = calls[0]?.[1]?.headers as Record<string, string>;
  const bodyHash = createHash("sha256").update('{"templateCode":"ops-kpi-daily"}').digest("hex");
  const canonical = `POST\n/api/internal/admin/report-jobs\n${bodyHash}\n1780000000\nU-ADMIN-001\nadmin`;
  assert.equal(headers["x-internal-call-signature"], createHmac("sha256", SECRET).update(canonical).digest("hex"));
  assert.equal(headers["x-actor-id"], "U-ADMIN-001");
  assert.equal(headers["x-operator-role"], "admin");
});

test("proxyAnalyticsRequest rejects affiliate before upstream call", async () => {
  let called = false;
  const response = await proxyAnalyticsRequest({
    viewer: { sub: "U-AFF-001", role: "affiliate", affiliateId: "AFF-001", iat: 1, exp: 2 },
    method: "GET",
    adminPath: "/api/internal/admin/report-templates",
    gatewayUrl: "http://gateway.local",
    secret: SECRET,
    fetchImpl: async () => {
      called = true;
      return new Response(null, { status: 200 });
    },
  });

  assert.equal(response.status, 403);
  assert.equal(called, false);
});

test("proxyAnalyticsRequest propagates upstream status and download headers", async () => {
  const response = await proxyAnalyticsRequest({
    viewer: { sub: "U-MGR-001", role: "manager", iat: 1, exp: 2 },
    method: "GET",
    adminPath: "/api/internal/admin/report-jobs/rpt-job-000001/exports/csv",
    gatewayUrl: "http://gateway.local",
    secret: SECRET,
    fetchImpl: async () =>
      new Response("csv", {
        status: 401,
        headers: {
          "content-type": "text/csv",
          "content-disposition": 'attachment; filename="rpt.csv"',
        },
      }),
  });

  assert.equal(response.status, 401);
  assert.equal(response.headers.get("content-type"), "text/csv");
  assert.equal(response.headers.get("content-disposition"), 'attachment; filename="rpt.csv"');
});
