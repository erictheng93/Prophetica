import "server-only";
import { randomUUID } from "node:crypto";
import { signInternalCall } from "./internal-call.js";
import type { SessionPayload } from "./session-crypto.js";
import {
  createMember,
  deleteMember,
  findMemberSummary,
  isMemberActionType,
  listMemberPendingCases,
  listMemberSummaries,
  updateMember,
  type MemberCreateRequest,
  type MemberActionRequest,
  type MemberActionResponse,
  type MemberUpdateRequest,
} from "./member-management.js";
import type {
  MemberPendingCase,
  MemberPendingCasesResponse,
} from "./member-management-shared.js";

export interface ProxyMemberListInput {
  viewer: SessionPayload | null;
}

export interface ProxyMemberPendingCasesInput {
  viewer: SessionPayload | null;
  searchParams?: URLSearchParams;
  gatewayUrl?: string;
  secret?: string;
  fetchImpl?: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
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

export async function proxyMemberPendingCasesRequest(
  input: ProxyMemberPendingCasesInput,
): Promise<Response> {
  if (!canReadMemberManagement(input.viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  const gatewayUrl = input.gatewayUrl ?? process.env.API_GATEWAY_URL;
  if (gatewayUrl) {
    const query = input.searchParams?.toString();
    const adminPath = `/api/internal/admin/members/pending-cases${query ? `?${query}` : ""}`;
    const signaturePath = adminPath.split("?")[0] ?? adminPath;
    const headers = signInternalCall({
      method: "GET",
      path: signaturePath,
      body: "",
      actorId: input.viewer.sub,
      role: input.viewer.role,
      secret: input.secret,
    });
    const upstream = await (input.fetchImpl ?? fetch)(
      new URL(adminPath, gatewayUrl),
      {
        method: "GET",
        headers: {
          ...headers,
          "x-request-id": crypto.randomUUID(),
        },
        cache: "no-store",
      },
    );
    if (upstream.ok) {
      const body = (await upstream.json()) as GatewayPendingCasesResponse;
      return Response.json(normalizeGatewayPendingCases(body), { status: 200 });
    }
    if (upstream.status !== 404 && upstream.status !== 501) {
      return Response.json(
        { error: "pending_cases_upstream_error" },
        { status: upstream.status },
      );
    }
  }

  return Response.json(
    listMemberPendingCases({
      query: input.searchParams?.get("q") ?? undefined,
      status: parseStatus(input.searchParams?.get("status")),
      restriction: input.searchParams?.get("restriction") ?? undefined,
      page: parsePositiveInt(input.searchParams?.get("page")),
      pageSize: parsePositiveInt(input.searchParams?.get("page_size")),
    }),
    { status: 200 },
  );
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

function parsePositiveInt(value: string | null | undefined): number | undefined {
  if (!value) return undefined;
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : undefined;
}

function parseStatus(value: string | null | undefined) {
  if (
    value === "inactive" ||
    value === "active" ||
    value === "restricted" ||
    value === "suspended" ||
    value === "all"
  ) {
    return value;
  }
  return undefined;
}

interface GatewayPendingCasesResponse {
  items: Array<{
    member_id: string;
    display_name: string;
    restriction_reason: string;
    contact: {
      email_masked: string;
      phone_masked: string;
    };
    status: string;
    kyc_status: string;
    affiliate_id?: string | null;
  }>;
  page: number;
  page_size: number;
  total: number;
}

function normalizeGatewayPendingCases(
  body: GatewayPendingCasesResponse,
): MemberPendingCasesResponse {
  return {
    items: body.items.map((item): MemberPendingCase => ({
      member_id: item.member_id,
      display_name: item.display_name,
      restriction_reason: item.restriction_reason,
      contact: {
        email_masked: item.contact.email_masked,
        phone_masked: item.contact.phone_masked,
      },
      status:
        item.status === "active" ||
        item.status === "restricted" ||
        item.status === "suspended"
          ? item.status
          : "inactive",
      kyc_status:
        item.kyc_status === "verified" ||
        item.kyc_status === "review_required"
          ? item.kyc_status
          : "pending",
      affiliate_id: item.affiliate_id ?? null,
    })),
    page: body.page,
    page_size: body.page_size,
    total: body.total,
    source: "api-gateway",
  };
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
