import type { SessionPayload } from "./session-crypto.js";
import type {
  KycStatus,
  MemberCreateRequest,
  MemberDeleteResponse,
  MemberManagementSummary,
  MemberSourceSummary,
  MemberUpdateRequest,
  RiskLevel,
} from "./member-management-shared.js";

type StoreResult<T> =
  | ({ ok: true } & T)
  | { ok: false; status: 400 | 403 | 404 | 409; error: string };

const affiliateNames: Record<string, string> = {
  "AFF-001": "AFF-001 / 北區代理",
  "AFF-014": "AFF-014 / 社群代理",
};

const initialSummaries: Record<string, MemberManagementSummary> = {
  "MEM-001": {
    member: {
      user_id: "MEM-001",
      display_name: "林雅婷",
      status: "active",
      tier: "Gold",
      registered_at: "2026-02-18T09:15:00+08:00",
      contact: {
        email_masked: "l***@example.com",
        phone_masked: "0912***678",
        line_id_masked: "lin***ops",
        country: "TW",
        timezone: "Asia/Taipei",
      },
    },
    account: {
      available_minor_units: -182450,
      locked_minor_units: -25000,
      frozen_minor_units: 0,
      currency: "POINTS",
      last_login_at: "2026-05-13T21:40:00+08:00",
      device_count: 3,
      two_factor_enabled: true,
    },
    kyc_risk: {
      kyc_status: "verified",
      risk_level: "low",
      risk_score: 18,
      last_reviewed_at: "2026-05-09T14:30:00+08:00",
      restrictions: [],
    },
    open_cases: [
      {
        case_id: "OPS-1037",
        case_type: "points_exception",
        status: "ready",
        owner_role: "Wallet Ops",
        age: "1h",
      },
    ],
    audit: {
      correlation_id: "admin-member-MEM-001",
      source: "admin:member-management",
      audit_required: true,
      latest_admin_action_at: "2026-05-13T18:20:00+08:00",
    },
    source: sourceForAffiliate("AFF-001"),
  },
  "MEM-1042": {
    member: {
      user_id: "MEM-1042",
      display_name: "陳柏安",
      status: "restricted",
      tier: "Standard",
      registered_at: "2026-04-02T16:05:00+08:00",
      contact: {
        email_masked: "c***@example.net",
        phone_masked: "0987***112",
        line_id_masked: null,
        country: "TW",
        timezone: "Asia/Taipei",
      },
    },
    account: {
      available_minor_units: -54000,
      locked_minor_units: -120000,
      frozen_minor_units: -18000,
      currency: "POINTS",
      last_login_at: "2026-05-14T08:10:00+08:00",
      device_count: 5,
      two_factor_enabled: false,
    },
    kyc_risk: {
      kyc_status: "review_required",
      risk_level: "high",
      risk_score: 86,
      last_reviewed_at: "2026-05-14T08:45:00+08:00",
      restrictions: ["withdrawals_locked", "manual_review_required"],
    },
    open_cases: [
      {
        case_id: "OPS-1042",
        case_type: "kyc_review",
        status: "new",
        owner_role: "Risk Ops",
        age: "24m",
      },
      {
        case_id: "OPS-1018",
        case_type: "manual_approval",
        status: "blocked",
        owner_role: "Ops Control",
        age: "5h",
      },
    ],
    audit: {
      correlation_id: "admin-member-MEM-1042",
      source: "admin:member-management",
      audit_required: true,
      latest_admin_action_at: null,
    },
    source: sourceForAffiliate("AFF-014"),
  },
  user_123: {
    member: {
      user_id: "user_123",
      display_name: "Member user_123",
      status: "inactive",
      tier: "Preview",
      registered_at: "2026-05-01T10:00:00+08:00",
      contact: {
        email_masked: "m***@example.com",
        phone_masked: "0900***123",
        line_id_masked: "mem***123",
        country: "TW",
        timezone: "Asia/Taipei",
      },
    },
    account: {
      available_minor_units: 0,
      locked_minor_units: 0,
      frozen_minor_units: 0,
      currency: "POINTS",
      last_login_at: null,
      device_count: 1,
      two_factor_enabled: false,
    },
    kyc_risk: {
      kyc_status: "pending",
      risk_level: "medium",
      risk_score: 52,
      last_reviewed_at: null,
      restrictions: ["kyc_pending"],
    },
    open_cases: [],
    audit: {
      correlation_id: "admin-member-user_123",
      source: "admin:member-management",
      audit_required: true,
      latest_admin_action_at: null,
    },
    source: sourceForAffiliate("AFF-001"),
  },
};

let summaries: Record<string, MemberManagementSummary> = cloneSummaries(initialSummaries);

export function resetMemberManagementStoreForTests(): void {
  summaries = cloneSummaries(initialSummaries);
}

export function listMemberSummaries(
  viewer: SessionPayload | null,
): MemberManagementSummary[] {
  if (!viewer) return [];
  return Object.values(summaries)
    .filter((summary) => canReadSummary(summary, viewer))
    .map(cloneSummary);
}

export function findMemberSummary(
  userId: string,
  viewer?: SessionPayload | null,
): MemberManagementSummary | null {
  const summary = summaries[userId.trim()];
  if (!summary) return null;
  if (viewer && !canReadSummary(summary, viewer)) return null;
  return cloneSummary(summary);
}

export function createMember(
  request: MemberCreateRequest,
  viewer: SessionPayload | null,
): StoreResult<{ summary: MemberManagementSummary }> {
  if (!canWriteMembers(viewer)) {
    return { ok: false, status: 403, error: "forbidden" };
  }

  const userId = request.user_id.trim();
  if (!userId || !request.display_name.trim() || !request.email.trim() || !request.phone.trim()) {
    return { ok: false, status: 400, error: "invalid_member" };
  }
  if (summaries[userId]) {
    return { ok: false, status: 409, error: "member_exists" };
  }

  const affiliateId = affiliateIdForWrite(request.affiliate_id, viewer);
  const summary: MemberManagementSummary = {
    member: {
      user_id: userId,
      display_name: request.display_name.trim(),
      status: "inactive",
      tier: request.tier.trim() || "Standard",
      registered_at: new Date().toISOString(),
      contact: {
        email_masked: maskEmail(request.email),
        phone_masked: maskPhone(request.phone),
        line_id_masked: request.line_id ? maskLineId(request.line_id) : null,
        country: request.country.trim() || "TW",
        timezone: request.timezone.trim() || "Asia/Taipei",
      },
    },
    account: {
      available_minor_units: 0,
      locked_minor_units: 0,
      frozen_minor_units: 0,
      currency: "POINTS",
      last_login_at: null,
      device_count: 0,
      two_factor_enabled: false,
    },
    kyc_risk: {
      kyc_status: "pending",
      risk_level: "medium",
      risk_score: 50,
      last_reviewed_at: null,
      restrictions: ["kyc_pending"],
    },
    open_cases: [],
    audit: auditFor(userId),
    source: sourceForAffiliate(affiliateId),
  };
  summaries[userId] = summary;
  return { ok: true, summary: cloneSummary(summary) };
}

export function updateMember(
  userId: string,
  request: MemberUpdateRequest,
  viewer: SessionPayload | null,
): StoreResult<{ summary: MemberManagementSummary }> {
  if (!canWriteMembers(viewer)) {
    return { ok: false, status: 403, error: "forbidden" };
  }
  const summary = summaries[userId.trim()];
  if (!summary) {
    return { ok: false, status: 404, error: "member_not_found" };
  }
  if (!canReadSummary(summary, viewer)) {
    return { ok: false, status: 403, error: "forbidden" };
  }

  if (typeof request.display_name === "string" && request.display_name.trim()) {
    summary.member.display_name = request.display_name.trim();
  }
  if (typeof request.tier === "string" && request.tier.trim()) {
    summary.member.tier = request.tier.trim();
  }
  if (typeof request.country === "string" && request.country.trim()) {
    summary.member.contact.country = request.country.trim();
  }
  if (typeof request.timezone === "string" && request.timezone.trim()) {
    summary.member.contact.timezone = request.timezone.trim();
  }
  if (typeof request.email === "string" && request.email.trim()) {
    summary.member.contact.email_masked = maskEmail(request.email);
  }
  if (typeof request.phone === "string" && request.phone.trim()) {
    summary.member.contact.phone_masked = maskPhone(request.phone);
  }
  if (typeof request.line_id === "string") {
    summary.member.contact.line_id_masked = request.line_id.trim()
      ? maskLineId(request.line_id)
      : null;
  } else if (request.line_id === null) {
    summary.member.contact.line_id_masked = null;
  }
  if (isKycStatus(request.kyc_status)) {
    summary.kyc_risk.kyc_status = request.kyc_status;
    summary.kyc_risk.last_reviewed_at = new Date().toISOString();
  }
  if (isRiskLevel(request.risk_level)) {
    summary.kyc_risk.risk_level = request.risk_level;
  }
  if (typeof request.risk_score === "number" && Number.isFinite(request.risk_score)) {
    summary.kyc_risk.risk_score = Math.max(0, Math.min(100, Math.round(request.risk_score)));
  }
  if (viewer.role !== "affiliate" && "affiliate_id" in request) {
    summary.source = sourceForAffiliate(request.affiliate_id ?? undefined);
  }
  applyKycActivation(summary);
  summary.audit.latest_admin_action_at = new Date().toISOString();

  return { ok: true, summary: cloneSummary(summary) };
}

export function deleteMember(
  userId: string,
  viewer: SessionPayload | null,
): StoreResult<MemberDeleteResponse> {
  if (viewer?.role !== "admin" && viewer?.role !== "manager") {
    return { ok: false, status: 403, error: "forbidden" };
  }
  const normalized = userId.trim();
  if (!summaries[normalized]) {
    return { ok: false, status: 404, error: "member_not_found" };
  }
  delete summaries[normalized];
  return { ok: true, user_id: normalized };
}

function canWriteMembers(
  viewer: SessionPayload | null,
): viewer is SessionPayload {
  return (
    viewer?.role === "admin" ||
    viewer?.role === "manager" ||
    (viewer?.role === "affiliate" && Boolean(viewer.affiliateId))
  );
}

function canReadSummary(
  summary: MemberManagementSummary,
  viewer: SessionPayload,
): boolean {
  if (viewer.role === "admin" || viewer.role === "manager") return true;
  return summary.source.affiliate_id === viewer.affiliateId;
}

function affiliateIdForWrite(
  requestedAffiliateId: string | undefined,
  viewer: SessionPayload,
): string | undefined {
  return viewer.role === "affiliate" ? viewer.affiliateId : requestedAffiliateId?.trim() || undefined;
}

function applyKycActivation(summary: MemberManagementSummary): void {
  const green =
    summary.kyc_risk.kyc_status === "verified" &&
    summary.kyc_risk.risk_level === "low";
  if (green) {
    summary.member.status = "active";
    summary.kyc_risk.restrictions = [];
    return;
  }
  if (summary.member.status === "active") {
    summary.member.status = "inactive";
  }
  if (summary.kyc_risk.kyc_status === "pending") {
    summary.kyc_risk.restrictions = ["kyc_pending"];
  }
}

function sourceForAffiliate(affiliateId?: string): MemberSourceSummary {
  const normalized = affiliateId?.trim();
  if (!normalized) {
    return { channel: "direct", affiliate_id: null, affiliate_name: "直接註冊" };
  }
  return {
    channel: "affiliate",
    affiliate_id: normalized,
    affiliate_name: affiliateNames[normalized] ?? normalized,
  };
}

function auditFor(userId: string): MemberManagementSummary["audit"] {
  return {
    correlation_id: `admin-member-${userId}`,
    source: "admin:member-management",
    audit_required: true,
    latest_admin_action_at: null,
  };
}

function maskEmail(value: string): string {
  const [local, domain] = value.trim().split("@");
  if (!local || !domain) return value.trim();
  return `${local.slice(0, 1)}***@${domain}`;
}

function maskPhone(value: string): string {
  const digits = value.trim();
  if (digits.length < 6) return digits;
  return `${digits.slice(0, 4)}***${digits.slice(-3)}`;
}

function maskLineId(value: string): string {
  const trimmed = value.trim();
  if (trimmed.length <= 3) return `${trimmed.slice(0, 1)}***`;
  return `${trimmed.slice(0, 3)}***${trimmed.slice(-2)}`;
}

function isKycStatus(value: unknown): value is KycStatus {
  return value === "verified" || value === "review_required" || value === "pending";
}

function isRiskLevel(value: unknown): value is RiskLevel {
  return value === "low" || value === "medium" || value === "high";
}

function cloneSummaries(
  value: Record<string, MemberManagementSummary>,
): Record<string, MemberManagementSummary> {
  return Object.fromEntries(
    Object.entries(value).map(([key, summary]) => [key, cloneSummary(summary)]),
  );
}

function cloneSummary(summary: MemberManagementSummary): MemberManagementSummary {
  return structuredClone(summary);
}
