import assert from "node:assert/strict";
import { test } from "node:test";
import {
  REFERRAL_ATTRIBUTION_COOKIE,
  REFERRAL_ATTRIBUTION_MAX_AGE_SECONDS,
  normalizeReferralCode,
  referralCodeFromSearchParams,
} from "../lib/referral-attribution.js";

test("referral attribution accepts supported query keys", () => {
  assert.equal(referralCodeFromSearchParams(new URLSearchParams("referral_code=AFF_123")), "AFF_123");
  assert.equal(referralCodeFromSearchParams(new URLSearchParams("ref=AFF-456")), "AFF-456");
  assert.equal(referralCodeFromSearchParams(new URLSearchParams("affiliate_code=AFF789")), "AFF789");
});

test("referral attribution rejects unsafe or empty codes", () => {
  assert.equal(normalizeReferralCode(""), null);
  assert.equal(normalizeReferralCode("ab"), null);
  assert.equal(normalizeReferralCode("AFF 123"), null);
  assert.equal(normalizeReferralCode("<script>"), null);
});

test("referral attribution cookie contract is first-touch compatible", () => {
  assert.equal(REFERRAL_ATTRIBUTION_COOKIE, "proph_referral_code");
  assert.equal(REFERRAL_ATTRIBUTION_MAX_AGE_SECONDS, 60 * 60 * 24 * 30);
});

