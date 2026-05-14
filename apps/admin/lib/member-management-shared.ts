export type MemberStatus = "inactive" | "active" | "restricted" | "suspended";
export type KycStatus = "verified" | "review_required" | "pending";
export type RiskLevel = "low" | "medium" | "high";

export type MemberActionType =
  | "add_note"
  | "request_kyc_review"
  | "restrict_withdrawals"
  | "escalate_case";

export interface MemberContactSummary {
  email_masked: string;
  phone_masked: string;
  line_id_masked: string | null;
  country: string;
  timezone: string;
}

export interface MemberIdentitySummary {
  user_id: string;
  display_name: string;
  status: MemberStatus;
  tier: string;
  registered_at: string;
  contact: MemberContactSummary;
}

export interface MemberAccountSummary {
  available_minor_units: number;
  locked_minor_units: number;
  frozen_minor_units: number;
  currency: string;
  last_login_at: string | null;
  device_count: number;
  two_factor_enabled: boolean;
}

export interface MemberKycRiskSummary {
  kyc_status: KycStatus;
  risk_level: RiskLevel;
  risk_score: number;
  last_reviewed_at: string | null;
  restrictions: string[];
}

export interface MemberOpenCaseSummary {
  case_id: string;
  case_type: string;
  status: "new" | "reviewing" | "blocked" | "ready";
  owner_role: string;
  age: string;
}

export interface MemberAuditSummary {
  correlation_id: string;
  source: string;
  audit_required: boolean;
  latest_admin_action_at: string | null;
}

export interface MemberSourceSummary {
  channel: "direct" | "affiliate";
  affiliate_id: string | null;
  affiliate_name: string;
}

export interface MemberManagementSummary {
  member: MemberIdentitySummary;
  account: MemberAccountSummary;
  kyc_risk: MemberKycRiskSummary;
  open_cases: MemberOpenCaseSummary[];
  audit: MemberAuditSummary;
  source: MemberSourceSummary;
}

export interface MemberCreateRequest {
  user_id: string;
  display_name: string;
  email: string;
  phone: string;
  line_id?: string;
  country: string;
  timezone: string;
  tier: string;
  affiliate_id?: string;
}

export interface MemberUpdateRequest {
  display_name?: string;
  email?: string;
  phone?: string;
  line_id?: string | null;
  country?: string;
  timezone?: string;
  tier?: string;
  affiliate_id?: string | null;
  kyc_status?: KycStatus;
  risk_level?: RiskLevel;
  risk_score?: number;
}

export interface MemberDeleteResponse {
  user_id: string;
}

export interface MemberPendingCase {
  member_id: string;
  display_name: string;
  restriction_reason: string;
  contact: {
    email_masked: string;
    phone_masked: string;
  };
  status: MemberStatus;
  kyc_status: KycStatus;
  affiliate_id: string | null;
}

export interface MemberPendingCasesQuery {
  query?: string;
  status?: MemberStatus | "all";
  restriction?: string;
  page?: number;
  pageSize?: number;
}

export interface MemberPendingCasesResponse {
  items: MemberPendingCase[];
  page: number;
  page_size: number;
  total: number;
  source: "mock" | "api-gateway";
}

export interface MemberActionRequest {
  target_user_id: string;
  action_type: MemberActionType;
  ticket_id: string;
  reason: string;
  correlation_id?: string;
}

export interface MemberActionResponse {
  action_id: string;
  target_user_id: string;
  action_type: MemberActionType;
  status: "submitted" | "pending_second_approval";
  requires_second_approval: boolean;
  audit: {
    actor_user_id: string;
    actor_role: string;
    target_user_id: string;
    ticket_id: string;
    correlation_id: string;
    ledger_transaction_posted: false;
  };
}

export const memberSearchHints = ["MEM-001", "MEM-1042", "user_123"] as const;

export function isMemberActionType(value: unknown): value is MemberActionType {
  return (
    value === "add_note" ||
    value === "request_kyc_review" ||
    value === "restrict_withdrawals" ||
    value === "escalate_case"
  );
}

export function formatMinorUnitsAsMemberPoints(minorUnits: number): string {
  const normalized = -minorUnits;
  const sign = normalized < 0 ? "-" : "";
  const absolute = Math.abs(normalized);
  const major = Math.floor(absolute / 100);
  const minor = String(absolute % 100).padStart(2, "0");
  return `${sign}${major.toLocaleString("zh-Hant")}.${minor} 點`;
}
