"use server";

// SPEC-005a §5.2 / §6.1 — Server actions invoked by Tier 2/3 client components.
// `startKycUpgradeAction` returns the StartKycUpgradeResponse so the client can
// pluck `sdkToken` and pass it to selectKycProvider; the previous redirect-based
// action prevented the iframe-mount flow defined in SPEC-005a §5.1.
// `refreshKycOverviewAction` powers the §6.1 polling loop (3s tick / 5min cap)
// while ws-gateway delivery (§6.2 Phase 2) is still gated on SPEC-008 §7.

import {
  fetchKycOverview,
  startKycUpgrade,
  type KycOverview,
} from "@/lib/kyc-api";
import { getViewer } from "@/lib/session";
import type { KycTargetTier, StartKycUpgradeResponse } from "@prophetica/ts-sdk";

export interface StartKycUpgradeActionInput {
  targetTier: KycTargetTier;
  declaredMonthlyIncome?: number | null;
  retryAttempt?: number;
}

export async function startKycUpgradeAction(
  input: StartKycUpgradeActionInput,
): Promise<StartKycUpgradeResponse> {
  const viewer = await getViewer();
  if (!viewer?.userId) {
    throw new Error("Unauthorized");
  }
  return startKycUpgrade(input, { userId: viewer.userId });
}

export async function refreshKycOverviewAction(): Promise<KycOverview> {
  const viewer = await getViewer();
  if (!viewer?.userId) {
    throw new Error("Unauthorized");
  }
  return fetchKycOverview({ userId: viewer.userId });
}

