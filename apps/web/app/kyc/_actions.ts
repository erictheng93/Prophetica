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
  type StartKycUpgradeInput,
} from "@/lib/kyc-api";
import type { StartKycUpgradeResponse } from "@prophetica/ts-sdk";

export async function startKycUpgradeAction(
  input: StartKycUpgradeInput,
): Promise<StartKycUpgradeResponse> {
  return startKycUpgrade(input);
}

export async function refreshKycOverviewAction(): Promise<KycOverview> {
  return fetchKycOverview();
}
