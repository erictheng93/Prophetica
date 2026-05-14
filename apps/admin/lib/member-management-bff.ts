import "server-only";
import { randomUUID } from "node:crypto";
import type { SessionPayload } from "./session-crypto.js";
import {
  createMember,
  deleteMember,
  findMemberSummary,
  isMemberActionType,
  listMemberSummaries,
  updateMember,
  type MemberCreateRequest,
  type MemberActionRequest,
  type MemberActionResponse,
  type MemberUpdateRequest,
} from "./member-management.js";

export interface ProxyMemberListInput {
  viewer: SessionPayload | null;
}

export interface ProxyMemberSummaryInput {
  viewer: SessionPayload | null;
  userId: string;
}

export interface ProxyMemberCreateInput {
  viewer: SessionPayload | null;
  request: unknown;
}

export interface ProxyMemberUpdateInput {
  viewer: SessionPayload | null;
  userId: string;
  request: unknown;
}

export interface ProxyMemberDeleteInput {
  viewer: SessionPayload | null;
  userId: string;
}

export interface ProxyMemberActionInput {
  viewer: SessionPayload | null;
  request: unknown;
}

export async function proxyMemberListRequest(
  input: ProxyMemberListInput,
): Promise<Response> {
  if (!canReadMemberManagement(input.viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  return Response.json({ members: listMemberSummaries(input.viewer) }, { status: 200 });
}

export async function proxyMemberSummaryRequest(
  input: ProxyMemberSummaryInput,
): Promise<Response> {
  if (!canReadMemberManagement(input.viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  const userId = input.userId.trim();
  if (!userId) {
    return Response.json({ error: "invalid_user_id" }, { status: 400 });
  }

  const summary = findMemberSummary(userId, input.viewer);
  if (!summary) {
    return Response.json({ error: "member_not_found" }, { status: 404 });
  }

  return Response.json(summary, { status: 200 });
}

export async function proxyMemberCreateRequest(
  input: ProxyMemberCreateInput,
): Promise<Response> {
  if (!canWriteMemberManagement(input.viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }
  const parsed = parseMemberCreate(input.request);
  if ("error" in parsed) {
    return Response.json({ error: parsed.error }, { status: 400 });
  }
  const result = createMember(parsed.value, input.viewer);
  if (!result.ok) {
    return Response.json({ error: result.error }, { status: result.status });
  }
  return Response.json(result.summary, { status: 201 });
}

export async function proxyMemberUpdateRequest(
  input: ProxyMemberUpdateInput,
): Promise<Response> {
  if (!canWriteMemberManagement(input.viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }
  const parsed = parseMemberUpdate(input.request);
  if ("error" in parsed) {
    return Response.json({ error: parsed.error }, { status: 400 });
  }
  const result = updateMember(input.userId, parsed.value, input.viewer);
  if (!result.ok) {
    return Response.json({ error: result.error }, { status: result.status });
  }
  return Response.json(result.summary, { status: 200 });
}

export async function proxyMemberDeleteRequest(
  input: ProxyMemberDeleteInput,
): Promise<Response> {
  const result = deleteMember(input.userId, input.viewer);
  if (!result.ok) {
    return Response.json({ error: result.error }, { status: result.status });
  }
  return Response.json(result, { status: 200 });
}

export async function proxyMemberActionRequest(
  input: ProxyMemberActionInput,
): Promise<Response> {
  if (!canUseMemberActions(input.viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  const parsed = parseMemberAction(input.request);
  if ("error" in parsed) {
    return Response.json({ error: parsed.error }, { status: 400 });
  }

  if (!findMemberSummary(parsed.value.target_user_id)) {
    return Response.json({ error: "member_not_found" }, { status: 404 });
  }

  const requiresSecondApproval = parsed.value.action_type !== "add_note";
  const correlationId =
    parsed.value.correlation_id?.trim() || `admin-member:${randomUUID()}`;
  const response: MemberActionResponse = {
    action_id: `mem-act-${randomUUID()}`,
    target_user_id: parsed.value.target_user_id,
    action_type: parsed.value.action_type,
    status: requiresSecondApproval ? "pending_second_approval" : "submitted",
    requires_second_approval: requiresSecondApproval,
    audit: {
      actor_user_id: input.viewer.sub,
      actor_role: input.viewer.role,
      target_user_id: parsed.value.target_user_id,
      ticket_id: parsed.value.ticket_id,
      correlation_id: correlationId,
      ledger_transaction_posted: false,
    },
  };

  return Response.json(response, { status: 202 });
}

function canReadMemberManagement(
  viewer: SessionPayload | null,
): viewer is SessionPayload {
  return viewer?.role === "admin" || viewer?.role === "manager" || viewer?.role === "affiliate";
}

function canWriteMemberManagement(
  viewer: SessionPayload | null,
): viewer is SessionPayload {
  return viewer?.role === "admin" || viewer?.role === "manager" || viewer?.role === "affiliate";
}

function canUseMemberActions(
  viewer: SessionPayload | null,
): viewer is SessionPayload {
  return viewer?.role === "admin" || viewer?.role === "manager";
}

function parseMemberCreate(
  value: unknown,
): { value: MemberCreateRequest } | { error: string } {
  if (!value || typeof value !== "object") {
    return { error: "invalid_request" };
  }
  const candidate = value as Record<string, unknown>;
  for (const key of [
    "user_id",
    "display_name",
    "email",
    "phone",
    "country",
    "timezone",
    "tier",
  ]) {
    if (typeof candidate[key] !== "string" || candidate[key].trim() === "") {
      return { error: `invalid_${key}` };
    }
  }

  return {
    value: {
      user_id: String(candidate.user_id).trim(),
      display_name: String(candidate.display_name).trim(),
      email: String(candidate.email).trim(),
      phone: String(candidate.phone).trim(),
      country: String(candidate.country).trim(),
      timezone: String(candidate.timezone).trim(),
      tier: String(candidate.tier).trim(),
      line_id:
        typeof candidate.line_id === "string" && candidate.line_id.trim()
          ? candidate.line_id.trim()
          : undefined,
      affiliate_id:
        typeof candidate.affiliate_id === "string" && candidate.affiliate_id.trim()
          ? candidate.affiliate_id.trim()
          : undefined,
    },
  };
}

function parseMemberUpdate(
  value: unknown,
): { value: MemberUpdateRequest } | { error: string } {
  if (!value || typeof value !== "object") {
    return { error: "invalid_request" };
  }
  const candidate = value as Record<string, unknown>;
  const result: MemberUpdateRequest = {};
  for (const key of [
    "display_name",
    "email",
    "phone",
    "country",
    "timezone",
    "tier",
    "affiliate_id",
  ] as const) {
    if (typeof candidate[key] === "string") {
      result[key] = candidate[key].trim();
    } else if (key === "affiliate_id" && candidate[key] === null) {
      result.affiliate_id = null;
    }
  }
  if (typeof candidate.line_id === "string") {
    result.line_id = candidate.line_id.trim();
  } else if (candidate.line_id === null) {
    result.line_id = null;
  }
  if (
    candidate.kyc_status === "verified" ||
    candidate.kyc_status === "review_required" ||
    candidate.kyc_status === "pending"
  ) {
    result.kyc_status = candidate.kyc_status;
  }
  if (
    candidate.risk_level === "low" ||
    candidate.risk_level === "medium" ||
    candidate.risk_level === "high"
  ) {
    result.risk_level = candidate.risk_level;
  }
  if (typeof candidate.risk_score === "number" && Number.isFinite(candidate.risk_score)) {
    result.risk_score = candidate.risk_score;
  }
  return { value: result };
}

function parseMemberAction(
  value: unknown,
): { value: MemberActionRequest } | { error: string } {
  if (!value || typeof value !== "object") {
    return { error: "invalid_request" };
  }

  const candidate = value as Record<string, unknown>;
  if (
    typeof candidate.target_user_id !== "string" ||
    candidate.target_user_id.trim() === ""
  ) {
    return { error: "invalid_target_user_id" };
  }
  if (!isMemberActionType(candidate.action_type)) {
    return { error: "invalid_action_type" };
  }
  if (
    typeof candidate.ticket_id !== "string" ||
    candidate.ticket_id.trim() === ""
  ) {
    return { error: "invalid_ticket_id" };
  }
  if (typeof candidate.reason !== "string" || candidate.reason.trim() === "") {
    return { error: "invalid_reason" };
  }

  return {
    value: {
      target_user_id: candidate.target_user_id.trim(),
      action_type: candidate.action_type,
      ticket_id: candidate.ticket_id.trim(),
      reason: candidate.reason.trim(),
      correlation_id:
        typeof candidate.correlation_id === "string"
          ? candidate.correlation_id.trim()
          : undefined,
    },
  };
}
