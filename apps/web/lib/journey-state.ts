import { fetchWalletOverview } from "./api-gateway";
import { fetchKycOverview } from "./kyc-api";
import { getUpstreamSessionToken, getViewer } from "./session";

export type JourneyStatus = "visitor" | "authenticated";

export interface JourneyState {
  status: JourneyStatus;
  userId?: string;
  kycTier?: number;
  balanceMinor?: number;
}

export async function getJourneyState(): Promise<JourneyState> {
  const viewer = await getViewer();
  if (!viewer) {
    return { status: "visitor" };
  }

  const bearerToken = await getUpstreamSessionToken();
  const [kycOverview, walletOverview] = await Promise.all([
    fetchKycOverview({ userId: viewer.userId }),
    fetchWalletOverview({
      userId: viewer.userId,
      bearerToken: bearerToken ?? undefined,
      allowUserIdFallback: process.env.NODE_ENV !== "production",
    }),
  ]);

  return {
    status: "authenticated",
    userId: viewer.userId,
    kycTier: kycOverview.currentTierNumber,
    balanceMinor: pointsToMinorUnits(walletOverview.balance.availablePoints),
  };
}

function pointsToMinorUnits(points: number): number {
  return Math.max(0, Math.round(points * 100));
}
