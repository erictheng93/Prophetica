// SPEC-005a §5.3 — Deterministic idempotency key for KYC upgrade requests.
//
// Why deterministic: the ts-sdk auto-generates UUIDv7 if no key is supplied
// (packages/ts-sdk/src/kyc/client.ts:91), so a double-click would create two
// pending records on the kyc-service side. The SPEC formula is
//   kyc:{userId}:{targetTier}:{currentTier1RecordId}
// and we degrade to a 3-segment form because KycStatusView does not yet
// expose the Tier 1 record ID. There is only one valid Tier 1 record per
// user, so 3 segments preserve the dedupe guarantee. Retries append
// `:retry{n}` per the SPEC §5.3 paragraph.

import type { KycTargetTier } from "@prophetica/ts-sdk";

export interface BuildKycIdempotencyKeyInput {
  userId: string;
  targetTier: KycTargetTier;
  tier1RecordId?: string | null;
  /** Bumped on every user-initiated retry. 0 = first attempt. */
  retryAttempt?: number;
}

export function buildKycIdempotencyKey(
  input: BuildKycIdempotencyKeyInput,
): string {
  const tier1RecordId = input.tier1RecordId?.trim();
  const base =
    tier1RecordId === undefined || tier1RecordId === ""
      ? `kyc:${input.userId}:${input.targetTier}`
      : `kyc:${input.userId}:${input.targetTier}:${tier1RecordId}`;
  if (!input.retryAttempt || input.retryAttempt <= 0) {
    return base;
  }
  return `${base}:retry${input.retryAttempt}`;
}
