import assert from "node:assert/strict";
import { test } from "node:test";

import { startKycUpgrade, type KycClientFactory } from "../lib/kyc-api.js";
import { buildKycIdempotencyKey } from "../lib/kyc-idempotency.js";

// SPEC-005a §10 acceptance #6 — "點兩次 submit 後端只建一筆 pending record".
//
// A full Playwright e2e is overkill: the property is the composition of three
// already-tested layers. This file proves the COMPOSITION wiring rather than
// the components in isolation:
//
//   1. buildKycIdempotencyKey is deterministic for (userId, targetTier).
//      → covered by tests/kyc-state-machine.test.ts.
//
//   2. startKycUpgrade builds the key server-side from the resolved userId,
//      so two invocations from a (possibly forged) client land at the same
//      key.  ← This file exercises that wiring.
//
//   3. kyc-service deduplicates on idempotency_key and returns
//      replayed: true on the second call.
//      → covered by services/kyc-service/src/lib.rs
//        (`start_upgrade_replays_same_idempotency_key_with_same_provider_session`).
//
// Together: a double-click cannot create two pending records.

test("two calls to startKycUpgrade with same (env userId, tier) hit kyc-service with the SAME idempotency key", async () => {
  const previousDevUserId = process.env.PROPHETICA_DEV_USER_ID;
  process.env.PROPHETICA_DEV_USER_ID = "user_doubleclick";
  try {
    const observed: string[] = [];
    const factory: KycClientFactory = () => ({
      async getStatus() {
        throw new Error("not used");
      },
      async verifyTierOne() {
        throw new Error("not used");
      },
      async startUpgrade(request) {
        observed.push(request.idempotencyKey ?? "<missing>");
        return {
          userId: request.userId,
          targetTier: request.targetTier,
          status: "pending",
          provider: "sumsub",
          providerApplicantId: "applicant_x",
          sdkToken: "sumsub_sandbox_user_doubleclick_TIER_2_1",
          expiresAt: null,
          replayed: observed.length > 1, // simulate kyc-service replay
        };
      },
    });

    await startKycUpgrade(
      { targetTier: "TIER_2" },
      { baseUrl: "http://kyc.local", kycClientFactory: factory },
    );
    await startKycUpgrade(
      { targetTier: "TIER_2" },
      { baseUrl: "http://kyc.local", kycClientFactory: factory },
    );

    assert.equal(observed.length, 2, "two SDK calls should have happened");
    assert.equal(observed[0], observed[1], "the two calls must share a key");
    assert.equal(
      observed[0],
      buildKycIdempotencyKey({
        userId: "user_doubleclick",
        targetTier: "TIER_2",
      }),
      "the key shape must follow SPEC-005a §5.3",
    );
  } finally {
    if (previousDevUserId === undefined) {
      delete process.env.PROPHETICA_DEV_USER_ID;
    } else {
      process.env.PROPHETICA_DEV_USER_ID = previousDevUserId;
    }
  }
});

test("retry attempt segment differentiates the key — explicit re-verify creates a new applicant", async () => {
  const previousDevUserId = process.env.PROPHETICA_DEV_USER_ID;
  process.env.PROPHETICA_DEV_USER_ID = "user_retry";
  try {
    const observed: string[] = [];
    const factory: KycClientFactory = () => ({
      async getStatus() {
        throw new Error("not used");
      },
      async verifyTierOne() {
        throw new Error("not used");
      },
      async startUpgrade(request) {
        observed.push(request.idempotencyKey ?? "<missing>");
        return {
          userId: request.userId,
          targetTier: request.targetTier,
          status: "pending",
          provider: "sumsub",
          providerApplicantId: "applicant_y",
          sdkToken: "sumsub_sandbox_user_retry_TIER_2_2",
          expiresAt: null,
          replayed: false,
        };
      },
    });

    await startKycUpgrade(
      { targetTier: "TIER_2", retryAttempt: 0 },
      { baseUrl: "http://kyc.local", kycClientFactory: factory },
    );
    await startKycUpgrade(
      { targetTier: "TIER_2", retryAttempt: 1 },
      { baseUrl: "http://kyc.local", kycClientFactory: factory },
    );

    assert.equal(observed.length, 2);
    assert.notEqual(
      observed[0],
      observed[1],
      "retry must produce a distinct key so backend creates a fresh applicant",
    );
    assert.equal(
      observed[0],
      "kyc:user_retry:TIER_2",
      "first attempt has no retry suffix",
    );
    assert.equal(
      observed[1],
      "kyc:user_retry:TIER_2:retry1",
      "explicit retry bumps the suffix per SPEC-005a §5.3",
    );
  } finally {
    if (previousDevUserId === undefined) {
      delete process.env.PROPHETICA_DEV_USER_ID;
    } else {
      process.env.PROPHETICA_DEV_USER_ID = previousDevUserId;
    }
  }
});
