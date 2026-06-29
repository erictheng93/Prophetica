import assert from "node:assert/strict";
import test from "node:test";

import { validateEvidenceShape } from "./preflight-cloudflare-admin-access-evidence.mjs";

const validLogEvidence = {
  sha256: "a".repeat(64),
  bytes: 1200,
  summary: "Apply complete",
};

const validTargetAccount = {
  account_id_sha256: "b".repeat(64),
  account_id_suffix: "abcdef",
  admin_app_domain: "admin.example.com",
  allowed_email_domains_sha256: "c".repeat(64),
  breakglass_email_sha256: "d".repeat(64),
  internal_service_token_name: "prophetica-admin-access",
};

test("Cloudflare preflight requires target-account metadata in recorded evidence", () => {
  const failures = validateEvidenceShape({
    plan_log: validLogEvidence,
    apply_log: validLogEvidence,
  });

  assert.deepEqual(failures, [
    "target_account is missing valid target-account metadata",
  ]);
});

test("Cloudflare preflight rejects non-hex log hashes", () => {
  const failures = validateEvidenceShape({
    target_account: validTargetAccount,
    plan_log: {
      ...validLogEvidence,
      sha256: "z".repeat(64),
    },
    apply_log: validLogEvidence,
  });

  assert.deepEqual(failures, [
    "plan_log.sha256 must be a 64-character hex digest",
  ]);
});

test("Cloudflare preflight accepts recorded evidence with target-account metadata", () => {
  const failures = validateEvidenceShape({
    target_account: validTargetAccount,
    plan_log: validLogEvidence,
    apply_log: validLogEvidence,
  });

  assert.deepEqual(failures, []);
});
