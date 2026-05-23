export type CompensationProposalStatus =
  | "proposed"
  | "approved"
  | "posting"
  | "posted"
  | "posting_failed"
  | "posting_unknown"
  | "rejected"
  | "cancelled";

export const visibleCompensationProposalStatuses = [
  "proposed",
  "approved",
  "posting",
  "posted",
  "posting_failed",
  "posting_unknown",
  "rejected",
] as const satisfies readonly CompensationProposalStatus[];

export type VisibleCompensationProposalStatus =
  (typeof visibleCompensationProposalStatuses)[number];

export type CompensationProposerKind =
  | "cs_system"
  | "manager"
  | "admin"
  | "affiliate";

export type CompensationReviewDecision = "approved" | "rejected";

export interface CompensationApprovalRequest {
  approver_user_id: string;
  role: string;
}

export interface CompensationProposal {
  proposal_id: string;
  user_id: string;
  ticket_id: string;
  reason: string;
  amount_minor_units: number;
  status: CompensationProposalStatus;
  proposer_kind: CompensationProposerKind;
  approvals: CompensationApprovalRequest[];
  ledger_transaction_posted: boolean;
  ledger_idempotency_key: string | null;
  ledger_transaction_id: number | null;
  posting_attempt_count: number;
  last_posting_error: string | null;
}

export interface CompensationAuditMetadata {
  action: string;
  subject_user_id: string;
  ticket_id: string;
  correlation_id: string;
  approval_roles: string[];
  proposer_kind: CompensationProposerKind;
  ledger_transaction_posted: boolean;
}

export interface CompensationAuditEventSummary {
  action: string;
  actor_user_id: string;
  actor_role: string;
  correlation_id: string;
  metadata: unknown;
  created_at: string;
}

export interface CompensationProposalCreateRequest {
  ticket_id?: string;
  reason?: string;
  amount_minor_units?: number;
}

export interface CompensationProposalCreateResponse {
  proposal: CompensationProposal;
  audit: CompensationAuditMetadata;
}

export interface CompensationProposalListResponse {
  proposals: CompensationProposal[];
  page: number;
  page_size: number;
  total: number;
  source: string;
}

export interface CompensationProposalDetailResponse {
  proposal: CompensationProposal;
  approvals: CompensationApprovalRequest[];
  recent_audit_events: CompensationAuditEventSummary[];
  posting_attempt_count: number;
  last_posting_error: string | null;
}

export interface CompensationReviewResponse {
  proposal: CompensationProposal;
  review: {
    reviewer_user_id: string;
    reviewer_role: string;
    decision: CompensationReviewDecision;
    note: string | null;
  };
  replayed: boolean;
}

export interface CompensationPostingRetryResponse {
  proposal: CompensationProposal;
  replayed: boolean;
}
