export type AffiliateStatus =
  | "active"
  | "review"
  | "frozen"
  | "onboarding"
  | "archived";
export type AffiliateRiskFlag =
  | "self-referral"
  | "burst-signup"
  | "geo-mismatch"
  | "kyc-tier1-only";
export type AffiliateCreditRating =
  | "A"
  | "B"
  | "C"
  | "restricted"
  | "suspended";
export type AffiliateCreditAccountStatus =
  | "pending_review"
  | "active"
  | "limited"
  | "frozen"
  | "closed";

export interface AffiliateSummary {
  id: string;
  userId?: string | null;
  displayName: string;
  emailMasked: string;
  status: AffiliateStatus;
  ownerSponsorId: string | null;
  commissionRateBps: number;
  tier: 1 | 2 | 3;
  subTreeSize: number;
  activeSubCount: number;
  pendingPayoutMinor: number;
  pendingPayoutCurrency: "USDT";
  lastActivityAt: string;
  riskFlags: AffiliateRiskFlag[];
}

export interface AffiliatePayoutRow {
  id: string;
  amountMinor: number;
  currency: "USDT";
  settledAt: string;
  status: "settled" | "pending" | "blocked";
}

export interface AffiliateRateChange {
  effectiveAt: string;
  previousBps: number;
  nextBps: number;
  changedBy: string;
  reason: string;
}

export interface AffiliateCreditAccountSummary {
  id: string;
  status: AffiliateCreditAccountStatus;
  creditLimitMinorUnits: number;
  usedMinorUnits: number;
  expiresAt: string | null;
}

export interface AffiliateCreditProfile {
  rating: AffiliateCreditRating;
  status: AffiliateCreditAccountStatus;
  maxActiveCreditAccounts: number;
  activeCreditAccounts: number;
  totalCreditLimitMinorUnits: number;
  perAccountLimitMinorUnits: number;
  reviewExpiresAt: string | null;
  lastChangedBy: string;
  lastChangedAt: string;
  auditCorrelationIds: string[];
  accounts: AffiliateCreditAccountSummary[];
}

export interface AffiliateDetail extends AffiliateSummary {
  subTreeBreakdown: {
    tier: 1 | 2 | 3;
    count: number;
    activeCount: number;
    mtdContributionMinor: number;
  }[];
  recentPayouts: AffiliatePayoutRow[];
  rateHistory: AffiliateRateChange[];
  auditCorrelationIds: string[];
  creditProfile: AffiliateCreditProfile;
}

export interface AffiliateMutationPayload {
  id?: string;
  userId?: string;
  affiliateId?: string;
  email?: string;
  phone?: string;
  password?: string;
  displayName?: string;
  status?: AffiliateStatus;
  ownerSponsorId?: string;
  managingManagerId?: string;
  commissionRateBps?: number;
  tier?: 1 | 2 | 3;
  creditRating?: AffiliateCreditRating;
  creditStatus?: AffiliateCreditAccountStatus;
  maxActiveCreditAccounts?: number;
  totalCreditLimitMinorUnits?: number;
  perAccountLimitMinorUnits?: number;
  creditReviewExpiresAt?: string;
  reason: string;
  authorizationBasis: string;
  effectiveAt?: string;
  expiresAt?: string;
}

export interface AffiliateMutationResponse {
  affiliate: AffiliateDetail;
  auditCorrelationId: string;
}

const affiliates: readonly AffiliateDetail[] = [
  {
    id: "AFF-001",
    displayName: "Crescent Partners",
    emailMasked: "c***@crescent.partners",
    status: "active",
    ownerSponsorId: null,
    commissionRateBps: 750,
    tier: 1,
    subTreeSize: 81,
    activeSubCount: 42,
    pendingPayoutMinor: 1_284_000_000,
    pendingPayoutCurrency: "USDT",
    lastActivityAt: "2026-05-09T18:42:00+08:00",
    riskFlags: [],
    subTreeBreakdown: [
      { tier: 1, count: 12, activeCount: 8, mtdContributionMinor: 612_000_000 },
      {
        tier: 2,
        count: 28,
        activeCount: 15,
        mtdContributionMinor: 415_200_000,
      },
      {
        tier: 3,
        count: 41,
        activeCount: 19,
        mtdContributionMinor: 256_800_000,
      },
    ],
    recentPayouts: [
      {
        id: "PAYOUT-2026-04-AFF-001",
        amountMinor: 982_400_000,
        currency: "USDT",
        settledAt: "2026-04-15",
        status: "settled",
      },
      {
        id: "PAYOUT-2026-03-AFF-001",
        amountMinor: 855_100_000,
        currency: "USDT",
        settledAt: "2026-03-15",
        status: "settled",
      },
      {
        id: "PAYOUT-2026-05-AFF-001",
        amountMinor: 1_284_000_000,
        currency: "USDT",
        settledAt: "2026-05-15",
        status: "pending",
      },
    ],
    rateHistory: [
      {
        effectiveAt: "2026-02-01",
        previousBps: 600,
        nextBps: 750,
        changedBy: "U-ADMIN-001",
        reason: "首期續約調整",
      },
    ],
    auditCorrelationIds: ["audit:affiliate-rate-change:AFF-001:2026-02-01"],
    creditProfile: {
      rating: "A",
      status: "active",
      maxActiveCreditAccounts: 24,
      activeCreditAccounts: 18,
      totalCreditLimitMinorUnits: 12_000_000,
      perAccountLimitMinorUnits: 1_000_000,
      reviewExpiresAt: "2026-08-01",
      lastChangedBy: "U-ADMIN-001",
      lastChangedAt: "2026-05-08T10:00:00+08:00",
      auditCorrelationIds: ["audit:affiliate-credit-rating:AFF-001:2026-05-08"],
      accounts: [
        {
          id: "CR-AFF-001-001",
          status: "active",
          creditLimitMinorUnits: 1_000_000,
          usedMinorUnits: 620_000,
          expiresAt: "2026-08-01",
        },
        {
          id: "CR-AFF-001-002",
          status: "limited",
          creditLimitMinorUnits: 500_000,
          usedMinorUnits: 410_000,
          expiresAt: "2026-07-15",
        },
      ],
    },
  },
  {
    id: "AFF-014",
    displayName: "Halcyon DMM",
    emailMasked: "o***@halcyon.dmm",
    status: "review",
    ownerSponsorId: "AFF-001",
    commissionRateBps: 500,
    tier: 2,
    subTreeSize: 36,
    activeSubCount: 11,
    pendingPayoutMinor: 412_500_000,
    pendingPayoutCurrency: "USDT",
    lastActivityAt: "2026-05-08T11:14:00+08:00",
    riskFlags: ["burst-signup"],
    subTreeBreakdown: [
      { tier: 1, count: 9, activeCount: 4, mtdContributionMinor: 196_000_000 },
      { tier: 2, count: 18, activeCount: 6, mtdContributionMinor: 145_600_000 },
      { tier: 3, count: 9, activeCount: 1, mtdContributionMinor: 70_900_000 },
    ],
    recentPayouts: [
      {
        id: "PAYOUT-2026-04-AFF-014",
        amountMinor: 318_700_000,
        currency: "USDT",
        settledAt: "2026-04-15",
        status: "settled",
      },
      {
        id: "PAYOUT-2026-05-AFF-014",
        amountMinor: 412_500_000,
        currency: "USDT",
        settledAt: "2026-05-15",
        status: "pending",
      },
    ],
    rateHistory: [],
    auditCorrelationIds: ["audit:affiliate-risk-flag:AFF-014:2026-05-08"],
    creditProfile: {
      rating: "C",
      status: "limited",
      maxActiveCreditAccounts: 6,
      activeCreditAccounts: 4,
      totalCreditLimitMinorUnits: 1_500_000,
      perAccountLimitMinorUnits: 300_000,
      reviewExpiresAt: "2026-06-08",
      lastChangedBy: "U-COMPLIANCE-002",
      lastChangedAt: "2026-05-08T11:30:00+08:00",
      auditCorrelationIds: ["audit:affiliate-credit-limit:AFF-014:2026-05-08"],
      accounts: [
        {
          id: "CR-AFF-014-001",
          status: "limited",
          creditLimitMinorUnits: 300_000,
          usedMinorUnits: 250_000,
          expiresAt: "2026-06-08",
        },
      ],
    },
  },
  {
    id: "AFF-022",
    displayName: "Northwind Referrals",
    emailMasked: "n***@northwind.io",
    status: "frozen",
    ownerSponsorId: "AFF-001",
    commissionRateBps: 0,
    tier: 2,
    subTreeSize: 14,
    activeSubCount: 0,
    pendingPayoutMinor: 0,
    pendingPayoutCurrency: "USDT",
    lastActivityAt: "2026-04-22T07:31:00+08:00",
    riskFlags: ["self-referral", "kyc-tier1-only"],
    subTreeBreakdown: [
      { tier: 1, count: 6, activeCount: 0, mtdContributionMinor: 0 },
      { tier: 2, count: 5, activeCount: 0, mtdContributionMinor: 0 },
      { tier: 3, count: 3, activeCount: 0, mtdContributionMinor: 0 },
    ],
    recentPayouts: [
      {
        id: "PAYOUT-2026-04-AFF-022",
        amountMinor: 51_000_000,
        currency: "USDT",
        settledAt: "2026-04-15",
        status: "blocked",
      },
    ],
    rateHistory: [
      {
        effectiveAt: "2026-04-22",
        previousBps: 500,
        nextBps: 0,
        changedBy: "U-ADMIN-001",
        reason: "Self-referral 命中，凍結待調查",
      },
    ],
    auditCorrelationIds: ["audit:affiliate-freeze:AFF-022:2026-04-22"],
    creditProfile: {
      rating: "suspended",
      status: "frozen",
      maxActiveCreditAccounts: 0,
      activeCreditAccounts: 0,
      totalCreditLimitMinorUnits: 0,
      perAccountLimitMinorUnits: 0,
      reviewExpiresAt: null,
      lastChangedBy: "U-ADMIN-001",
      lastChangedAt: "2026-04-22T07:31:00+08:00",
      auditCorrelationIds: ["audit:affiliate-credit-freeze:AFF-022:2026-04-22"],
      accounts: [
        {
          id: "CR-AFF-022-001",
          status: "frozen",
          creditLimitMinorUnits: 0,
          usedMinorUnits: 0,
          expiresAt: null,
        },
      ],
    },
  },
  {
    id: "AFF-103",
    displayName: "Stellar Onboard",
    emailMasked: "s***@stellar.partners",
    status: "onboarding",
    ownerSponsorId: null,
    commissionRateBps: 600,
    tier: 1,
    subTreeSize: 0,
    activeSubCount: 0,
    pendingPayoutMinor: 0,
    pendingPayoutCurrency: "USDT",
    lastActivityAt: "2026-05-09T09:05:00+08:00",
    riskFlags: [],
    subTreeBreakdown: [
      { tier: 1, count: 0, activeCount: 0, mtdContributionMinor: 0 },
      { tier: 2, count: 0, activeCount: 0, mtdContributionMinor: 0 },
      { tier: 3, count: 0, activeCount: 0, mtdContributionMinor: 0 },
    ],
    recentPayouts: [],
    rateHistory: [],
    auditCorrelationIds: [],
    creditProfile: {
      rating: "restricted",
      status: "pending_review",
      maxActiveCreditAccounts: 0,
      activeCreditAccounts: 0,
      totalCreditLimitMinorUnits: 0,
      perAccountLimitMinorUnits: 0,
      reviewExpiresAt: "2026-05-23",
      lastChangedBy: "system.onboarding",
      lastChangedAt: "2026-05-09T09:05:00+08:00",
      auditCorrelationIds: [],
      accounts: [],
    },
  },
];

export function getAffiliatesSnapshot(): AffiliateSummary[] {
  return affiliates.map(
    ({
      subTreeBreakdown: _b,
      recentPayouts: _p,
      rateHistory: _r,
      auditCorrelationIds: _a,
      creditProfile: _c,
      ...summary
    }) => summary,
  );
}

export function getAffiliateDetail(id: string): AffiliateDetail | undefined {
  return affiliates.find((row) => row.id === id);
}

export function formatBps(bps: number): string {
  if (!Number.isFinite(bps) || bps <= 0) return "0%";
  return `${(bps / 100).toFixed(2)}%`;
}

export function formatMinor(minor: number, currency: "USDT"): string {
  const major = minor / 1_000_000;
  return `${currency} ${major.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}

export function formatTwdMinor(minor: number): string {
  const major = minor / 100;
  return `TWD ${major.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;
}
