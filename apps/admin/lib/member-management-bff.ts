import "server-only";
import { randomUUID } from "node:crypto";
import type { SessionPayload } from "./session-crypto.js";
import {
  findMemberSummary,
  isMemberActionType,
  type MemberActionRequest,
  type MemberActionResponse,
} from "./member-management.js";

export interface ProxyMemberSummaryInput {
  viewer: SessionPayload | null;
  userId: string;
}

export interface ProxyMemberActionInput {
  viewer: SessionPayload | null;
  request: unknown;
}

export async function proxyMemberSummaryRequest(
  input: ProxyMemberSummaryInput,
): Promise<Response> {
  if (!canUseMemberManagement(input.viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  const userId = input.userId.trim();
  if (!userId) {
    return Response.json({ error: "invalid_user_id" }, { status: 400 });
  }

  const summary = findMemberSummary(userId);
  if (!summary) {
    return Response.json({ error: "member_not_found" }, { status: 404 });
  }

  return Response.json(summary, { status: 200 });
}

export async function proxyMemberActionRequest(
  input: ProxyMemberActionInput,
): Promise<Response> {
  if (!canUseMemberManagement(input.viewer)) {
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

function canUseMemberManagement(
  viewer: SessionPayload | null,
): viewer is SessionPayload {
  return viewer?.role === "admin" || viewer?.role === "manager";
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
