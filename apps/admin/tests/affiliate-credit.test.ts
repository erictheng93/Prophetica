import assert from "node:assert/strict";
import { test } from "node:test";
import { formatTwdMinor, getAffiliateDetail } from "../lib/affiliates.js";

test("affiliate detail exposes credit rating and credit account controls data", () => {
  const active = getAffiliateDetail("AFF-001");
  assert.equal(active?.creditProfile.rating, "A");
  assert.equal(active?.creditProfile.status, "active");
  assert.equal(active?.creditProfile.activeCreditAccounts, 18);
  assert.ok(
    active?.creditProfile.auditCorrelationIds.includes(
      "audit:affiliate-credit-rating:AFF-001:2026-05-08",
    ),
  );

  const frozen = getAffiliateDetail("AFF-022");
  assert.equal(frozen?.creditProfile.rating, "suspended");
  assert.equal(frozen?.creditProfile.maxActiveCreditAccounts, 0);
  assert.equal(frozen?.creditProfile.accounts[0]?.status, "frozen");
});

test("formatTwdMinor renders credit limits with two decimals", () => {
  assert.equal(formatTwdMinor(1_234_500), "TWD 12,345.00");
  assert.equal(formatTwdMinor(0), "TWD 0.00");
});
