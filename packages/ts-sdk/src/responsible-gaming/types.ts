export type SelfExclusionMode = "timeout" | "permanent";
export type SelfExclusionPeriod = "six_months" | "one_year";
export type PointsDisposition = "hold" | "donate";

export interface SelfExclusionRecord {
  recordId: string;
  userId: string;
  mode: SelfExclusionMode;
  period: SelfExclusionPeriod | null;
  pointsDisposition: PointsDisposition | null;
  reason: string | null;
  startsAtEpochSeconds: number;
  endsAtEpochSeconds: number | null;
  frozenMinorUnits: bigint;
}

export interface SelfExclusionStatus {
  userId: string;
  active: boolean;
  record: SelfExclusionRecord | null;
  source: string;
}

export interface SubmitSelfExclusionRequest {
  mode: SelfExclusionMode;
  period?: SelfExclusionPeriod;
  pointsDisposition?: PointsDisposition;
  consent: boolean;
  reason?: string;
  idempotencyKey?: string;
}

export interface SubmitSelfExclusionResponse {
  record: SelfExclusionRecord;
  ledgerIdempotencyKey: string;
  replayed: boolean;
}
