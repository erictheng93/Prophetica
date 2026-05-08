import assert from "node:assert/strict";
import { test } from "node:test";

import { createKycClient } from "../src/index.js";
import type { Fetcher } from "../src/index.js";

test("kyc client reads current user status with auth context", async () => {
  const calls: Array<{ url: string; headers: Record<string, string> }> = [];
  const fetcher: Fetcher = async (input, init) => {
    calls.push({
      url: String(input),
      headers: Object.fromEntries(new Headers(init?.headers).entries()),
    });

    return Response.json({
      user_id: "user_42",
      current_tier: "TIER_2",
      status: "pending",
      provider: "sumsub",
      provider_applicant_id: "applicant_42",
      country: "TW",
      tier1_record_id: "tier1_record_42",
      declared_monthly_income: 6000000,
      submitted_at: "unix:1",
      reviewed_at: null,
      expires_at: null,
    });
  };
  const client = createKycClient({
    baseUrl: "http://kyc.local/",
    auth: { userId: "user_42" },
    fetcher,
  });

  const status = await client.getStatus("user_42");

  assert.equal(calls[0]?.url, "http://kyc.local/api/v1/kyc/users/user_42/status");
  assert.equal(calls[0]?.headers["x-user-id"], "user_42");
  assert.equal(status.currentTier, "TIER_2");
  assert.equal(status.status, "pending");
  assert.equal(status.country, "TW");
  assert.equal(status.tier1RecordId, "tier1_record_42");
  assert.equal(status.declaredMonthlyIncome, 6000000);
});

test("kyc client starts upgrade and verifies tier one with idempotency keys", async () => {
  const calls: Array<{ url: string; body: unknown }> = [];
  const fetcher: Fetcher = async (input, init) => {
    calls.push({
      url: String(input),
      body: JSON.parse(String(init?.body)),
    });

    if (String(input).endsWith("/tier-1")) {
      return Response.json(
        {
          user_id: "user_42",
          target_tier: "TIER_1",
          status: "passed",
          provider: "manual",
          document_id_hash: "a".repeat(64),
          replayed: false,
        },
        { status: 201 },
      );
    }

    return Response.json(
      {
        user_id: "user_42",
        target_tier: "TIER_2",
        status: "pending",
        provider: "sumsub",
        provider_applicant_id: "applicant_42",
        sdk_token: "sumsub_sandbox_user_42_TIER_2_1",
        expires_at: null,
        replayed: false,
      },
      { status: 201 },
    );
  };
  const client = createKycClient({
    baseUrl: "http://kyc.local",
    fetcher,
  });

  const tierOne = await client.verifyTierOne({
    userId: "user_42",
    idempotencyKey: "kyc:user_42:tier1:001",
    legalName: "Test User",
    birthday: "1990-01-02",
    country: "TW",
    documentId: "A123456789",
    consent: true,
  });
  const upgrade = await client.startUpgrade({
    userId: "user_42",
    targetTier: "TIER_2",
    idempotencyKey: "kyc:user_42:tier2:001",
    declaredMonthlyIncome: 6000000,
  });

  assert.equal(tierOne.status, "passed");
  assert.equal(tierOne.documentIdHash.length, 64);
  assert.equal(upgrade.sdkToken, "sumsub_sandbox_user_42_TIER_2_1");
  assert.deepEqual(calls[0]?.body, {
    idempotency_key: "kyc:user_42:tier1:001",
    legal_name: "Test User",
    birthday: "1990-01-02",
    country: "TW",
    document_id: "A123456789",
    consent: true,
  });
  assert.deepEqual(calls[1]?.body, {
    target_tier: "TIER_2",
    idempotency_key: "kyc:user_42:tier2:001",
    declared_monthly_income: 6000000,
  });
});
