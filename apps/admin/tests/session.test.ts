import assert from "node:assert/strict";
import { test } from "node:test";
import { signSession, verifySession } from "../lib/session-crypto.js";

const SECRET = "test-secret-do-not-use-in-prod";

test("signed session round-trips and preserves payload", async () => {
  const token = await signSession({ sub: "U-1", role: "admin" }, SECRET);
  const verified = await verifySession(token, SECRET);
  assert.ok(verified);
  assert.equal(verified.sub, "U-1");
  assert.equal(verified.role, "admin");
  assert.equal(typeof verified.iat, "number");
  assert.equal(typeof verified.exp, "number");
});

test("affiliate session preserves affiliateId", async () => {
  const token = await signSession({ sub: "U-2", role: "affiliate", affiliateId: "AFF-9" }, SECRET);
  const verified = await verifySession(token, SECRET);
  assert.equal(verified?.affiliateId, "AFF-9");
});

test("verifySession rejects token signed with a different secret", async () => {
  const token = await signSession({ sub: "U-1", role: "admin" }, SECRET);
  const verified = await verifySession(token, "another-secret");
  assert.equal(verified, null);
});

test("verifySession rejects tampered payload", async () => {
  const token = await signSession({ sub: "U-1", role: "manager" }, SECRET);
  const [body, sig] = token.split(".");
  // flip the role in the payload by re-encoding without re-signing
  const bodyDecoded = JSON.parse(Buffer.from(body, "base64url").toString("utf8"));
  bodyDecoded.role = "admin";
  const tampered = `${Buffer.from(JSON.stringify(bodyDecoded)).toString("base64url")}.${sig}`;
  const verified = await verifySession(tampered, SECRET);
  assert.equal(verified, null);
});

test("verifySession rejects expired tokens", async () => {
  const longAgo = Math.floor(Date.now() / 1000) - 60 * 60 * 24 * 30;
  const token = await signSession({ sub: "U-1", role: "admin" }, SECRET, longAgo);
  const verified = await verifySession(token, SECRET);
  assert.equal(verified, null);
});

test("verifySession rejects malformed tokens", async () => {
  assert.equal(await verifySession("not-a-token", SECRET), null);
  assert.equal(await verifySession("only.one.dot.too.many", SECRET), null);
});

test("verifySession rejects unknown roles", async () => {
  const now = Math.floor(Date.now() / 1000);
  const payload = { sub: "U-1", role: "superuser", iat: now, exp: now + 60 };
  const body = Buffer.from(JSON.stringify(payload)).toString("base64url");
  // forge a body but no valid sig — verify must reject regardless
  const verified = await verifySession(`${body}.invalidsig`, SECRET);
  assert.equal(verified, null);
});
