import assert from "node:assert/strict";
import { test } from "node:test";
import { proxyKycDocumentRequest } from "../lib/kyc-documents-bff.js";

test("proxyKycDocumentRequest streams body without buffering", async () => {
  const stream = new ReadableStream({
    start(c) { c.enqueue(new Uint8Array([1, 2, 3])); c.close(); },
  });
  const fetchImpl = async () =>
    new Response(stream, { status: 200, headers: { "content-type": "image/png" } });
  const res = await proxyKycDocumentRequest(
    { sub: "U-ADMIN", role: "admin" } as never,
    { userId: "U-1", adminPath: "/api/internal/admin/members/U-1/kyc-documents/doc_1" },
    { gatewayUrl: "https://gw.test", secret: "shared-secret-32-bytes-1234567890", fetchImpl },
  );
  assert.equal(res.status, 200);
  assert.equal(res.headers.get("content-type"), "image/png");
  assert.ok(res.body instanceof ReadableStream);
});
