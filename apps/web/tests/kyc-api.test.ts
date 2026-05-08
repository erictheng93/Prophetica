import assert from "node:assert/strict";
import { test } from "node:test";

import {
  KYC_FALLBACK_USER_ID,
  fetchKycOverview,
  startKycUpgrade,
  submitTierOneVerification,
  tierMonthlyLimitPoints,
  type KycClientFactory,
} from "../lib/kyc-api.js";

test("tierMonthlyLimitPoints maps SPEC-005 tier caps", () => {
  assert.equal(tierMonthlyLimitPoints("TIER_0"), 0);
  assert.equal(tierMonthlyLimitPoints("TIER_1"), 30_000);
  assert.equal(tierMonthlyLimitPoints("TIER_2"), 300_000);
  assert.equal(tierMonthlyLimitPoints("TIER_3"), null);
});

test("fetchKycOverview calls kyc-service status facade with auth context", async () => {
  const overview = await fetchKycOverview({
    baseUrl: "http://kyc.local",
    userId: "user_42",
    kycClientFactory: createFetchBackedKycClient,
  });

  assert.equal(overview.userId, "user_42");
  assert.equal(overview.currentTier, "TIER_2");
  assert.equal(overview.currentTierNumber, 2);
  assert.equal(overview.status, "pending");
  assert.equal(overview.monthlyLimitPoints, 300_000);
  assert.equal(overview.source, "kyc-service");
});

test("fetchKycOverview returns Tier 0 fallback when kyc-service is unavailable", async () => {
  const overview = await fetchKycOverview({
    baseUrl: "http://kyc.local",
    userId: KYC_FALLBACK_USER_ID,
    kycClientFactory: () => ({
      async getStatus() {
        throw new Error("connection refused");
      },
      async verifyTierOne() {
        throw new Error("not used");
      },
      async startUpgrade() {
        throw new Error("not used");
      },
    }),
  });

  assert.equal(overview.currentTier, "TIER_0");
  assert.equal(overview.status, "pending");
  assert.equal(overview.source, "fallback:kyc-service-unavailable");
});

test("submit helpers call kyc client mutations for server actions", async () => {
  const calls: unknown[] = [];
  const factory: KycClientFactory = () => ({
    async getStatus() {
      throw new Error("not used");
    },
    async verifyTierOne(request) {
      calls.push(request);
      return {
        userId: request.userId,
        targetTier: "TIER_1",
        status: "passed",
        provider: "manual",
        documentIdHash: "b".repeat(64),
        replayed: false,
      };
    },
    async startUpgrade(request) {
      calls.push(request);
      return {
        userId: request.userId,
        targetTier: request.targetTier,
        status: "pending",
        provider: "sumsub",
        providerApplicantId: "applicant_42",
        sdkToken: "sumsub_sandbox_user_42_TIER_2_1",
        expiresAt: null,
        replayed: false,
      };
    },
  });

  await submitTierOneVerification(
    {
      userId: "user_42",
      legalName: "Test User",
      birthday: "1990-01-02",
      country: "TW",
      documentId: "A123456789",
      consent: true,
    },
    { baseUrl: "http://kyc.local", kycClientFactory: factory },
  );
  await startKycUpgrade(
    {
      userId: "user_42",
      targetTier: "TIER_2",
      declaredMonthlyIncome: 6000000,
      idempotencyKey: "kyc:user_42:TIER_2",
    },
    { baseUrl: "http://kyc.local", kycClientFactory: factory },
  );

  assert.equal(calls.length, 2);
  assert.deepEqual(calls[0], {
    userId: "user_42",
    legalName: "Test User",
    birthday: "1990-01-02",
    country: "TW",
    documentId: "A123456789",
    consent: true,
  });
  assert.deepEqual(calls[1], {
    userId: "user_42",
    targetTier: "TIER_2",
    declaredMonthlyIncome: 6000000,
    idempotencyKey: "kyc:user_42:TIER_2",
  });
});

const createFetchBackedKycClient: KycClientFactory = ({ auth }) => ({
  async getStatus(userId) {
    assert.equal(auth?.userId, "user_42");
    assert.equal(userId, "user_42");
    return {
      userId: "user_42",
      currentTier: "TIER_2",
      status: "pending",
      provider: "sumsub",
      providerApplicantId: "applicant_42",
      declaredMonthlyIncome: 6000000,
      submittedAt: "unix:1",
      reviewedAt: null,
      expiresAt: null,
    };
  },
  async verifyTierOne() {
    throw new Error("not used");
  },
  async startUpgrade() {
    throw new Error("not used");
  },
});
