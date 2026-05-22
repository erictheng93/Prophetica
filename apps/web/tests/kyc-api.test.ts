import assert from "node:assert/strict";
import { test } from "node:test";

import {
  KYC_FALLBACK_USER_ID,
  fetchKycOverview,
  resolveKycServiceBaseUrl,
  resolveKycSubmitErrorStatus,
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
  assert.equal(overview.country, "TW");
  assert.equal(overview.tier1RecordId, "tier1_record_42");
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
  assert.equal(overview.country, null);
  assert.equal(overview.tier1RecordId, null);
  assert.equal(overview.source, "fallback:kyc-service-unavailable");
});

test("resolveKycServiceBaseUrl accepts both KYC env names with one default port", () => {
  const previousBaseUrl = process.env.KYC_SERVICE_BASE_URL;
  const previousUrl = process.env.KYC_SERVICE_URL;
  try {
    delete process.env.KYC_SERVICE_BASE_URL;
    delete process.env.KYC_SERVICE_URL;
    assert.equal(resolveKycServiceBaseUrl(), "http://localhost:8085");

    process.env.KYC_SERVICE_URL = "http://kyc-url.local/";
    assert.equal(resolveKycServiceBaseUrl(), "http://kyc-url.local");

    process.env.KYC_SERVICE_BASE_URL = "http://kyc-base.local/";
    assert.equal(resolveKycServiceBaseUrl(), "http://kyc-base.local");
    assert.equal(
      resolveKycServiceBaseUrl("http://explicit.local/"),
      "http://explicit.local",
    );
  } finally {
    restoreEnv("KYC_SERVICE_BASE_URL", previousBaseUrl);
    restoreEnv("KYC_SERVICE_URL", previousUrl);
  }
});

test("submit helpers use KYC_SERVICE_URL when KYC_SERVICE_BASE_URL is absent", async () => {
  const previousBaseUrl = process.env.KYC_SERVICE_BASE_URL;
  const previousUrl = process.env.KYC_SERVICE_URL;
  const previousDevUserId = process.env.PROPHETICA_DEV_USER_ID;
  const seenBaseUrls: string[] = [];
  const factory: KycClientFactory = ({ baseUrl }) => {
    seenBaseUrls.push(baseUrl);
    return {
      async getStatus() {
        throw new Error("not used");
      },
      async verifyTierOne(request) {
        return {
          userId: request.userId,
          targetTier: "TIER_1",
          status: "passed",
          provider: "manual",
          documentIdHash: "c".repeat(64),
          replayed: false,
        };
      },
      async startUpgrade() {
        throw new Error("not used");
      },
    };
  };

  try {
    delete process.env.KYC_SERVICE_BASE_URL;
    process.env.KYC_SERVICE_URL = "http://kyc-url.local/";
    process.env.PROPHETICA_DEV_USER_ID = "user_env";

    await submitTierOneVerification(
      {
        legalName: "Test User",
        birthday: "1990-01-02",
        country: "TW",
        documentId: "A123456789",
        consent: true,
      },
      { kycClientFactory: factory },
    );

    assert.deepEqual(seenBaseUrls, ["http://kyc-url.local"]);
  } finally {
    restoreEnv("KYC_SERVICE_BASE_URL", previousBaseUrl);
    restoreEnv("KYC_SERVICE_URL", previousUrl);
    restoreEnv("PROPHETICA_DEV_USER_ID", previousDevUserId);
  }
});

test("resolveKycSubmitErrorStatus maps submit failures to user-facing states", () => {
  assert.equal(
    resolveKycSubmitErrorStatus(apiError(409, "duplicate_document")),
    "duplicate_document",
  );
  assert.equal(
    resolveKycSubmitErrorStatus(apiError(422, "validation_failed")),
    "invalid_submission",
  );
  assert.equal(
    resolveKycSubmitErrorStatus(apiError(500, "kyc_store_unavailable")),
    "service_unavailable",
  );
  assert.equal(
    resolveKycSubmitErrorStatus(new TypeError("fetch failed")),
    "service_unavailable",
  );
});

test("submit helpers call kyc client mutations for server actions", async () => {
  const previousDevUserId = process.env.PROPHETICA_DEV_USER_ID;
  process.env.PROPHETICA_DEV_USER_ID = "user_42";
  const calls: unknown[] = [];
  const factory: KycClientFactory = () => ({
    async getStatus() {
      return {
        userId: "user_42",
        currentTier: "TIER_1",
        status: "passed",
        provider: "manual",
        providerApplicantId: null,
        country: "TW",
        tier1RecordId: "tier1_record_42",
        declaredMonthlyIncome: null,
        submittedAt: "unix:1",
        reviewedAt: "unix:1",
        expiresAt: null,
      };
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

  try {
    await submitTierOneVerification(
      {
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
        targetTier: "TIER_2",
        declaredMonthlyIncome: 6000000,
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
      idempotencyKey: "kyc:user_42:TIER_2:tier1_record_42",
    });
  } finally {
    if (previousDevUserId === undefined) {
      delete process.env.PROPHETICA_DEV_USER_ID;
    } else {
      process.env.PROPHETICA_DEV_USER_ID = previousDevUserId;
    }
  }
});

test("startKycUpgrade derives user identity server-side instead of trusting input", async () => {
  const previousDevUserId = process.env.PROPHETICA_DEV_USER_ID;
  process.env.PROPHETICA_DEV_USER_ID = "server_user_42";
  const calls: unknown[] = [];
  const factory: KycClientFactory = ({ auth }) => ({
    async getStatus() {
      throw new Error("not used");
    },
    async verifyTierOne() {
      throw new Error("not used");
    },
    async startUpgrade(request) {
      calls.push({ authUserId: auth?.userId, request });
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

  try {
    const forgedInput = {
      userId: "attacker_supplied",
      targetTier: "TIER_2" as const,
    } as unknown as Parameters<typeof startKycUpgrade>[0];

    await startKycUpgrade(forgedInput, {
      baseUrl: "http://kyc.local",
      kycClientFactory: factory,
    });

    assert.deepEqual(calls, [
      {
        authUserId: "server_user_42",
        request: {
          userId: "server_user_42",
          targetTier: "TIER_2",
          declaredMonthlyIncome: undefined,
          idempotencyKey: "kyc:server_user_42:TIER_2",
        },
      },
    ]);
  } finally {
    if (previousDevUserId === undefined) {
      delete process.env.PROPHETICA_DEV_USER_ID;
    } else {
      process.env.PROPHETICA_DEV_USER_ID = previousDevUserId;
    }
  }
});

test("startKycUpgrade falls back to 3-segment idempotency key when status is unavailable", async () => {
  const previousDevUserId = process.env.PROPHETICA_DEV_USER_ID;
  process.env.PROPHETICA_DEV_USER_ID = "user_42";
  const calls: unknown[] = [];
  const factory: KycClientFactory = () => ({
    async getStatus() {
      throw new Error("connection refused");
    },
    async verifyTierOne() {
      throw new Error("not used");
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

  try {
    await startKycUpgrade(
      {
        targetTier: "TIER_2",
      },
      { baseUrl: "http://kyc.local", kycClientFactory: factory },
    );

    assert.deepEqual(calls, [
      {
        userId: "user_42",
        targetTier: "TIER_2",
        declaredMonthlyIncome: undefined,
        idempotencyKey: "kyc:user_42:TIER_2",
      },
    ]);
  } finally {
    if (previousDevUserId === undefined) {
      delete process.env.PROPHETICA_DEV_USER_ID;
    } else {
      process.env.PROPHETICA_DEV_USER_ID = previousDevUserId;
    }
  }
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
      country: "TW",
      tier1RecordId: "tier1_record_42",
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

function restoreEnv(name: string, value: string | undefined): void {
  if (value === undefined) {
    delete process.env[name];
  } else {
    process.env[name] = value;
  }
}

function apiError(status: number, code: string): unknown {
  return {
    name: "ApiError",
    status,
    envelope: {
      code,
      message: code,
    },
  };
}
