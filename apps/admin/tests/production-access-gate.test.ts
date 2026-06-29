import assert from "node:assert/strict";
import { test } from "node:test";
import { hasTrustedProductionAccessSignal } from "../lib/production-access-gate.js";

test("production internal gate rejects spoofed raw mTLS header without trusted proxy secret", () => {
  const headers = new Headers({
    "x-internal-mtls": "true",
  });

  assert.equal(
    hasTrustedProductionAccessSignal(headers, {
      trustedProxySecret: "shared-secret-32-bytes-1234567890",
    }),
    false,
  );
});

test("production internal gate rejects unconfigured trusted proxy secret", () => {
  const headers = new Headers({
    "x-admin-trusted-proxy-secret": "shared-secret-32-bytes-1234567890",
    "x-internal-mtls": "true",
  });

  assert.equal(
    hasTrustedProductionAccessSignal(headers, {
      trustedProxySecret: "",
    }),
    false,
  );
});

test("production internal gate accepts matching trusted proxy secret", () => {
  const headers = new Headers({
    "x-admin-trusted-proxy-secret": "shared-secret-32-bytes-1234567890",
  });

  assert.equal(
    hasTrustedProductionAccessSignal(headers, {
      trustedProxySecret: "shared-secret-32-bytes-1234567890",
    }),
    true,
  );
});

test("production internal gate rejects mismatched trusted proxy secret", () => {
  const headers = new Headers({
    "x-admin-trusted-proxy-secret": "spoofed-secret",
  });

  assert.equal(
    hasTrustedProductionAccessSignal(headers, {
      trustedProxySecret: "shared-secret-32-bytes-1234567890",
    }),
    false,
  );
});
