import "server-only";
import type { MemberManagementSummary } from "./member-management-shared.js";

export type {
  KycStatus,
  MemberAccountSummary,
  MemberActionRequest,
  MemberActionResponse,
  MemberActionType,
  MemberAuditSummary,
  MemberContactSummary,
  MemberIdentitySummary,
  MemberKycRiskSummary,
  MemberManagementSummary,
  MemberOpenCaseSummary,
  MemberStatus,
  RiskLevel,
} from "./member-management-shared.js";
export {
  formatMinorUnitsAsMemberPoints,
  isMemberActionType,
  memberSearchHints,
} from "./member-management-shared.js";

const summaries: Record<string, MemberManagementSummary> = {
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
  },
  user_123: {
    member: {
      user_id: "user_123",
      display_name: "Member user_123",
      status: "active",
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
  },
};

export function findMemberSummary(userId: string): MemberManagementSummary | null {
  const normalized = userId.trim();
  return summaries[normalized] ?? null;
}
