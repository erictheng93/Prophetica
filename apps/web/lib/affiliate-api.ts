// SPEC-011 v0.2.1 §5.1 / §9.2 — affiliate dashboard frontend API client.
//
// Hits the api-gateway public route added in services/api-gateway/src/lib.rs:
//   GET /v1/account/affiliate?limit=N
// Prefer the user-service bearer session for gateway auth. x-user-id remains
// only as a transitional fallback for local callers that have not migrated.
//
// Falls back to an empty dashboard when the gateway is unavailable so the
// page can still render its disclaimers and CTAs.

const DEFAULT_API_GATEWAY_BASE_URL = "http://127.0.0.1:8080";
const DEFAULT_DEV_USER_ID = "user_01";
const DEFAULT_REFERRAL_LIMIT = 25;
const POINTS_PER_TWD = 100; // SPEC-002 §3.1: 1 點 = TWD 1; 100 minor units = 1 點.

export type AffiliateReferralStatus =
  | "pending"
  | "qualified"
  | "disqualified"
  | "pending_review"
  | "pending_eddreview";

export type AffiliateDashboardSource =
  | "api-gateway"
  | "fallback:api-gateway-unavailable"
  | "fallback:affiliate-not-found";

export interface AffiliateReferralListItem {
  referredId: string;
  maskedUsername: string;
  referredAtDay: number;
  status: AffiliateReferralStatus;
  cumulativeCommissionPoints: number;
}

export interface AffiliateCommissionSummary {
  successfulReferralsCount: number;
  signupBonusPoints: number;
  tradeCommissionPoints: number;
  totalRewardsPoints: number;
  last30dCommissionPoints: number;
  last12mCommissionPoints: number;
}

export interface AffiliateDashboard {
  userId: string;
  referralCode: string;
  shareUrl: string;
  summary: AffiliateCommissionSummary;
  referrals: AffiliateReferralListItem[];
  source: AffiliateDashboardSource;
}

interface ApiAffiliateReferral {
  referred_id: string;
  masked_username: string;
  referred_at_day: number;
  status: string;
  cumulative_commission_minor_units: number;
}

interface ApiAffiliateSummary {
  successful_referrals_count: number;
  signup_bonus_minor_units: number;
  trade_commission_minor_units: number;
  total_rewards_minor_units: number;
  last_30d_commission_minor_units: number;
  last_12m_commission_minor_units: number;
}

interface ApiAffiliateDashboardResponse {
  user_id: string;
  referral_code: string;
  share_url: string;
  summary: ApiAffiliateSummary;
  referrals: ApiAffiliateReferral[];
}

type Fetcher = (input: string | URL, init?: RequestInit) => Promise<Response>;

export interface FetchAffiliateDashboardOptions {
  baseUrl?: string;
  userId?: string;
  bearerToken?: string;
  allowUserIdFallback?: boolean;
  fetcher?: Fetcher;
  limit?: number;
}

const KNOWN_STATUSES: AffiliateReferralStatus[] = [
  "pending",
  "qualified",
  "disqualified",
  "pending_review",
  "pending_eddreview",
];

function normalizeStatus(input: string): AffiliateReferralStatus {
  return KNOWN_STATUSES.includes(input as AffiliateReferralStatus)
    ? (input as AffiliateReferralStatus)
    : "pending";
}

function trimTrailingSlash(value: string): string {
  return value.endsWith("/") ? value.slice(0, -1) : value;
}

function minorUnitsToPoints(minorUnits: number): number {
  return minorUnits / POINTS_PER_TWD;
}

function emptyDashboard(
  userId: string,
  source: AffiliateDashboardSource,
): AffiliateDashboard {
  return {
    userId,
    referralCode: "",
    shareUrl: "",
    summary: {
      successfulReferralsCount: 0,
      signupBonusPoints: 0,
      tradeCommissionPoints: 0,
      totalRewardsPoints: 0,
      last30dCommissionPoints: 0,
      last12mCommissionPoints: 0,
    },
    referrals: [],
    source,
  };
}

export async function fetchAffiliateDashboard({
  baseUrl = process.env.API_GATEWAY_BASE_URL ?? DEFAULT_API_GATEWAY_BASE_URL,
  userId = process.env.PROPHETICA_DEV_USER_ID ?? DEFAULT_DEV_USER_ID,
  bearerToken,
  allowUserIdFallback,
  fetcher = fetch,
  limit = DEFAULT_REFERRAL_LIMIT,
}: FetchAffiliateDashboardOptions = {}): Promise<AffiliateDashboard> {
  const url = new URL(
    `/v1/account/affiliate?limit=${limit}`,
    trimTrailingSlash(baseUrl) + "/",
  );
  if (!bearerToken && !allowUserIdFallback) {
    return emptyDashboard(userId, "fallback:api-gateway-unavailable");
  }

  try {
    const headers = new Headers({ accept: "application/json" });
    if (bearerToken) {
      headers.set("authorization", `Bearer ${bearerToken}`);
    } else {
      headers.set("x-user-id", userId);
    }
    const response = await fetcher(url, {
      cache: "no-store",
      headers,
    });

    if (response.status === 404) {
      return emptyDashboard(userId, "fallback:affiliate-not-found");
    }
    if (!response.ok) {
      throw new Error(`api-gateway request failed: ${response.status}`);
    }

    const payload = (await response.json()) as ApiAffiliateDashboardResponse;
    return {
      userId: payload.user_id,
      referralCode: payload.referral_code,
      shareUrl: payload.share_url,
      summary: mapSummary(payload.summary),
      referrals: payload.referrals.map(mapReferral),
      source: "api-gateway",
    };
  } catch {
    return emptyDashboard(userId, "fallback:api-gateway-unavailable");
  }
}

function mapSummary(summary: ApiAffiliateSummary): AffiliateCommissionSummary {
  return {
    successfulReferralsCount: summary.successful_referrals_count,
    signupBonusPoints: minorUnitsToPoints(summary.signup_bonus_minor_units),
    tradeCommissionPoints: minorUnitsToPoints(
      summary.trade_commission_minor_units,
    ),
    totalRewardsPoints: minorUnitsToPoints(summary.total_rewards_minor_units),
    last30dCommissionPoints: minorUnitsToPoints(
      summary.last_30d_commission_minor_units,
    ),
    last12mCommissionPoints: minorUnitsToPoints(
      summary.last_12m_commission_minor_units,
    ),
  };
}

function mapReferral(
  referral: ApiAffiliateReferral,
): AffiliateReferralListItem {
  return {
    referredId: referral.referred_id,
    maskedUsername: referral.masked_username,
    referredAtDay: referral.referred_at_day,
    status: normalizeStatus(referral.status),
    cumulativeCommissionPoints: minorUnitsToPoints(
      referral.cumulative_commission_minor_units,
    ),
  };
}
