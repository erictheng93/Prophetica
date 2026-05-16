import assert from "node:assert/strict";
import { test } from "node:test";
import { hasRecentHardwareMfa, requiresStepUpMfa } from "../lib/step-up-mfa-policy.js";

test("step-up MFA policy only applies to mutating high-sensitivity paths", () => {
  assert.equal(requiresStepUpMfa("/approvals", "GET"), false);
  assert.equal(requiresStepUpMfa("/approvals", "POST"), true);
  assert.equal(requiresStepUpMfa("/api/members/actions", "POST"), true);
  assert.equal(requiresStepUpMfa("/api/points/manual-adjustments", "PATCH"), true);
  assert.equal(requiresStepUpMfa("/dashboard", "POST"), false);
});

test("step-up MFA requires recent hardware authentication", () => {
  const now = 1778881800;

  assert.equal(
    hasRecentHardwareMfa({ sub: "U-1", role: "admin", iat: now - 60, exp: now + 60 }, now),
    false,
  );
  assert.equal(
    hasRecentHardwareMfa(
      { sub: "U-1", role: "admin", iat: now - 60, exp: now + 60, mfaAt: now - 60, amr: ["pwd"] },
      now,
    ),
    false,
  );
  assert.equal(
    hasRecentHardwareMfa(
      { sub: "U-1", role: "admin", iat: now - 60, exp: now + 60, mfaAt: now - 1801, amr: ["pwd", "hwk"] },
      now,
    ),
    false,
  );
  assert.equal(
    hasRecentHardwareMfa(
      { sub: "U-1", role: "admin", iat: now - 60, exp: now + 60, mfaAt: now - 1800, amr: ["pwd", "hwk"] },
      now,
    ),
    true,
  );
});

