import { cache } from "react";
import { signInternalCall } from "./internal-call.js";
import type { SessionPayload } from "./session-crypto.js";
import {
  allowsAffiliateMockFallback,
  getAffiliateDetail,
  getAffiliatesSnapshot,
  type AffiliateMutationPayload,
  type AffiliateMutationResponse,
  type AffiliateDetail,
  type AffiliateSummary,
} from "./affiliates.js";

export async function fetchAdminAffiliateList(
  viewer: SessionPayload,
): Promise<AffiliateSummary[]> {
  if (viewer.role !== "admin" && viewer.role !== "manager") return [];
  const response = await proxyAffiliateGatewayRequest(viewer, {
    method: "GET",
    adminPath: "/api/internal/admin/affiliates",
  });
  if (response?.ok) return (await response.json()) as AffiliateSummary[];
  return allowsAffiliateMockFallback() ? getAffiliatesSnapshot() : [];
}

export async function fetchAdminAffiliateDetail(
  viewer: SessionPayload,
  affiliateId: string,
): Promise<AffiliateDetail | null> {
  if (viewer.role !== "admin" && viewer.role !== "manager") return null;
  const response = await proxyAffiliateGatewayRequest(viewer, {
    method: "GET",
    adminPath: `/api/internal/admin/affiliates/${encodeURIComponent(affiliateId)}`,
  });
  if (response?.ok) return (await response.json()) as AffiliateDetail;
  return allowsAffiliateMockFallback()
    ? (getAffiliateDetail(affiliateId) ?? null)
    : null;
}

export async function createAdminAffiliate(
  viewer: SessionPayload,
  payload: AffiliateMutationPayload,
): Promise<AffiliateMutationResponse | null> {
  if (viewer.role !== "admin" && viewer.role !== "manager") return null;
  const response = await proxyAffiliateGatewayRequest(viewer, {
    method: "POST",
    adminPath: "/api/internal/admin/affiliates",
    body: JSON.stringify(payload),
  });
  if (!response?.ok) return null;
  return (await response.json()) as AffiliateMutationResponse;
}

export async function updateAdminAffiliate(
  viewer: SessionPayload,
  affiliateId: string,
  payload: AffiliateMutationPayload,
): Promise<AffiliateMutationResponse | null> {
  if (viewer.role !== "admin" && viewer.role !== "manager") return null;
  const response = await proxyAffiliateGatewayRequest(viewer, {
    method: "PATCH",
    adminPath: `/api/internal/admin/affiliates/${encodeURIComponent(affiliateId)}`,
    body: JSON.stringify(payload),
  });
  if (!response?.ok) return null;
  return (await response.json()) as AffiliateMutationResponse;
}

export async function archiveAdminAffiliate(
  viewer: SessionPayload,
  affiliateId: string,
  payload: Pick<AffiliateMutationPayload, "reason" | "authorizationBasis">,
): Promise<AffiliateMutationResponse | null> {
  if (viewer.role !== "admin" && viewer.role !== "manager") return null;
  const response = await proxyAffiliateGatewayRequest(viewer, {
    method: "DELETE",
    adminPath: `/api/internal/admin/affiliates/${encodeURIComponent(affiliateId)}`,
    body: JSON.stringify(payload),
  });
  if (!response?.ok) return null;
  return (await response.json()) as AffiliateMutationResponse;
}

export async function fetchCommissionAdjustmentReviews(
  viewer: SessionPayload,
): Promise<AffiliateCommissionAdjustmentReview[]> {
  if (viewer.role !== "admin" && viewer.role !== "manager") return [];
  const response = await proxyAffiliateGatewayRequest(viewer, {
    method: "GET",
    adminPath: "/api/internal/admin/affiliate-commission-adjustments",
  });
  if (!response?.ok) return [];
  return (await response.json()) as AffiliateCommissionAdjustmentReview[];
}

export async function fetchAffiliatePayoutReviews(
  viewer: SessionPayload,
): Promise<AffiliatePayoutReview[]> {
  if (viewer.role !== "admin" && viewer.role !== "manager") return [];
  const response = await proxyAffiliateGatewayRequest(viewer, {
    method: "GET",
    adminPath: "/api/internal/admin/affiliate-payouts",
  });
  if (!response?.ok) return [];
  return (await response.json()) as AffiliatePayoutReview[];
}

export async function fetchAffiliateDashboard(
  viewer: SessionPayload,
): Promise<AffiliatePortalDashboard | null> {
  return fetchAffiliateDashboardForViewer(viewer.sub, viewer.role);
}

const fetchAffiliateDashboardForViewer = cache(
  async function fetchAffiliateDashboardForViewer(
    sub: string,
    role: SessionPayload["role"],
  ): Promise<AffiliatePortalDashboard | null> {
    if (role !== "affiliate") return null;
    const response = await proxyAccountGatewayRequest(
      sub,
      "/v1/account/affiliate?limit=200",
    );
    if (!response?.ok) return null;
    return (await response.json()) as AffiliatePortalDashboard;
  },
);

async function proxyAffiliateGatewayRequest(
  viewer: SessionPayload,
  input: { method: string; adminPath: string; body?: string },
): Promise<Response | null> {
  const gatewayUrl = process.env.API_GATEWAY_URL;
  if (!gatewayUrl) return null;
  const body = input.body ?? "";
  const signaturePath = input.adminPath.split("?")[0] ?? input.adminPath;
  const headers = signInternalCall({
    method: input.method,
    path: signaturePath,
    body,
    actorId: viewer.sub,
    role: viewer.role,
  });
  return fetch(new URL(input.adminPath, gatewayUrl), {
    method: input.method,
    headers: {
      ...headers,
      "x-request-id": crypto.randomUUID(),
      ...(body ? { "content-type": "application/json" } : {}),
    },
    body: body || undefined,
    cache: "no-store",
  }).catch(() => null);
}

async function proxyAccountGatewayRequest(
  sub: string,
  path: string,
): Promise<Response | null> {
  const gatewayUrl = process.env.API_GATEWAY_URL;
  if (!gatewayUrl) return null;
  return fetch(new URL(path, gatewayUrl), {
    method: "GET",
    headers: {
      "x-user-id": sub,
      "x-request-id": crypto.randomUUID(),
    },
    cache: "no-store",
  }).catch(() => null);
}

export interface AffiliatePortalDashboard {
  user_id: string;
  referral_code: string;
  share_url: string;
  summary: {
    successful_referrals_count: number;
    signup_bonus_minor_units: number;
    trade_commission_minor_units: number;
    total_rewards_minor_units: number;
    last_30d_commission_minor_units: number;
    last_12m_commission_minor_units: number;
  };
  referrals: Array<{
    referred_id: string;
    masked_username: string;
    referred_at_day: number;
    status: string;
    cumulative_commission_minor_units: number;
  }>;
}

export interface AffiliateCommissionAdjustmentReview {
  id: string;
  affiliateId: string;
  adjustmentType: "credit" | "debit" | string;
  amountMinor: number;
  currency: "USDT";
  reason: string;
  status: string;
  requestedBy: string;
  requestedAt: string;
}

export interface AffiliatePayoutReview {
  id: string;
  affiliateId: string;
  amountMinor: number;
  currency: "USDT";
  requestedAt: string;
  status: string;
}
