import assert from "node:assert/strict";
import { createHash, createHmac } from "node:crypto";
import { test } from "node:test";
import { signInternalCall } from "../lib/internal-call.js";

const SECRET = "shared-secret-32-bytes-1234567890";

test("signInternalCall returns canonical headers with HMAC-SHA256 lowercase hex", () => {
  const headers = signInternalCall({
    method: "POST",
    path: "/api/internal/admin/report-jobs",
    body: '{"hi":"there"}',
    actorId: "U-ADMIN-001",
    role: "admin",
    secret: SECRET,
    nowSeconds: 1_780_000_000,
  });

  const bodyHash = createHash("sha256").update('{"hi":"there"}').digest("hex");
  const canonical = `POST\n/api/internal/admin/report-jobs\n${bodyHash}\n1780000000\nU-ADMIN-001\nadmin`;
  const expected = createHmac("sha256", SECRET).update(canonical).digest("hex");

  assert.equal(headers["x-actor-id"], "U-ADMIN-001");
  assert.equal(headers["x-operator-role"], "admin");
  assert.equal(headers["x-internal-call-timestamp"], "1780000000");
  assert.equal(headers["x-internal-call-signature"], expected);
});

test("signInternalCall handles empty body deterministically", () => {
  const first = signInternalCall({
    method: "GET",
    path: "/x",
    body: "",
    actorId: "U-1",
    role: "manager",
    secret: SECRET,
    nowSeconds: 1,
  });
  const second = signInternalCall({
    method: "GET",
    path: "/x",
    body: "",
    actorId: "U-1",
    role: "manager",
    secret: SECRET,
    nowSeconds: 1,
  });

  assert.equal(first["x-internal-call-signature"], second["x-internal-call-signature"]);
  assert.match(first["x-internal-call-signature"], /^[0-9a-f]{64}$/);
});
