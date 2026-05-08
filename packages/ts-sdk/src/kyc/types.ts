export type KycTier = "TIER_0" | "TIER_1" | "TIER_2" | "TIER_3";
export type KycStatus = "pending" | "passed" | "rejected" | "expired" | "frozen";

export interface KycStatusView {
  userId: string;
  currentTier: KycTier;
  status: KycStatus;
  provider: string | null;
  providerApplicantId: string | null;
  country: string | null;
  tier1RecordId: string | null;
  declaredMonthlyIncome: number | null;
  submittedAt: string | null;
  reviewedAt: string | null;
  expiresAt: string | null;
}

export interface VerifyTierOneRequest {
  userId: string;
  idempotencyKey?: string;
  legalName: string;
  birthday: string;
  country: string;
  documentId: string;
  consent: boolean;
}

export interface VerifyTierOneResponse {
  userId: string;
  targetTier: KycTier;
  status: KycStatus;
  provider: string;
  documentIdHash: string;
  replayed: boolean;
}

export interface StartKycUpgradeRequest {
  userId: string;
  targetTier: Exclude<KycTier, "TIER_0">;
  idempotencyKey?: string;
  declaredMonthlyIncome?: number | null;
}

export interface StartKycUpgradeResponse {
  userId: string;
  targetTier: KycTier;
  status: KycStatus;
  provider: string;
  providerApplicantId: string | null;
  sdkToken: string;
  expiresAt: string | null;
  replayed: boolean;
}
