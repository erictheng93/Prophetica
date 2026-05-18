import "server-only";
import { randomUUID } from "node:crypto";
import { signInternalCall } from "./internal-call.js";
import { allowsMemberMockFallback } from "./member-management-runtime.js";
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
  type MemberActionListResponse,
  type MemberActionRequest,
  type MemberActionResponse,
  type MemberUpdateRequest,
} from "./member-management.js";
import type {
  MemberManagementSummary,
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
  fetchImpl?: (
    input: RequestInfo | URL,
    init?: RequestInit,
  ) => Promise<Response>;
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

export interface ProxyMemberActionListInput {
  viewer: SessionPayload | null;
  searchParams?: URLSearchParams;
}

export interface ProxyMemberActionReviewInput {
  viewer: SessionPayload | null;
  actionId: string;
  request: unknown;
}

export async function proxyMemberListRequest(
  input: ProxyMemberListInput,
): Promise<Response> {
  if (!canReadMemberManagement(input.viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  const gatewayResponse = await proxyMemberGatewayRequest(input.viewer, {
    method: "GET",
    adminPath: memberListAdminPath(input.viewer),
  });
  if (gatewayResponse) return gatewayResponse;

  return Response.json(
    { members: listMemberSummaries(input.viewer) },
    { status: 200 },
  );
}

export async function proxyMemberPendingCasesRequest(
  input: ProxyMemberPendingCasesInput,
): Promise<Response> {
  if (!canReadMemberManagement(input.viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  const gatewayUrl = input.gatewayUrl ?? process.env.API_GATEWAY_URL;
  const allowMockFallback = allowsMemberMockFallback();
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
    let upstream: Response;
    try {
      upstream = await (input.fetchImpl ?? fetch)(
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
    } catch {
      if (allowMockFallback) {
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
      return Response.json({ error: "gateway_unavailable" }, { status: 502 });
    }
    if (upstream.ok) {
      const body = (await upstream.json()) as GatewayPendingCasesResponse;
      return Response.json(normalizeGatewayPendingCases(body), { status: 200 });
    }
    if (upstream.status !== 404 && upstream.status !== 501) {
      if (upstream.status === 502) {
        return Response.json(
          { error: "pending_cases_upstream_unavailable" },
          { status: 502 },
        );
      }
      return Response.json(
        { error: "pending_cases_upstream_error" },
        { status: upstream.status },
      );
    }
    if (!allowMockFallback) {
      return Response.json(
        { error: "pending_cases_upstream_unimplemented" },
        { status: 502 },
      );
    }
  } else if (!allowMockFallback) {
    return Response.json({ error: "gateway_unconfigured" }, { status: 500 });
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

  const gatewayResponse = await proxyMemberGatewayRequest(input.viewer, {
    method: "GET",
    adminPath: `/api/internal/admin/members/${encodeURIComponent(userId)}/summary`,
  });
  if (gatewayResponse) {
    if (!gatewayResponse.ok || input.viewer.role !== "affiliate") {
      return gatewayResponse;
    }
    const summary = (await gatewayResponse.json()) as MemberManagementSummary;
    if (!canReadGatewaySummary(summary, input.viewer)) {
      return Response.json({ error: "member_not_found" }, { status: 404 });
    }
    return Response.json(summary, { status: 200 });
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
  const request =
    input.viewer.role === "affiliate"
      ? { ...parsed.value, affiliate_id: input.viewer.affiliateId }
      : parsed.value;
  const gatewayCreateRequest = request.affiliate_id
    ? {
        ...request,
        registration_audit: {
          source: "apps-admin:member-create-bff",
          referral_capture_required: true,
          referral_code: request.affiliate_id,
          referred_user_id: request.user_id,
          actor_user_id: input.viewer.sub,
          actor_role: input.viewer.role,
        },
      }
    : request;
  const gatewayResponse = await proxyMemberGatewayRequest(input.viewer, {
    method: "POST",
    adminPath: "/api/internal/admin/members",
    body: JSON.stringify(gatewayCreateRequest),
  });
  if (gatewayResponse) {
    if (gatewayResponse.ok && request.affiliate_id) {
      const captureResponse = await proxyReferralCaptureRequest(input.viewer, {
        referralCode: request.affiliate_id,
        referredUserId: request.user_id,
      });
      if (captureResponse && !captureResponse.ok) {
        return Response.json(
          { error: "referral_capture_failed" },
          { status: captureResponse.status },
        );
      }
    }
    return gatewayResponse;
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
  const gatewaySummary = await fetchGatewayMemberSummary(
    input.viewer,
    input.userId,
  );
  if (gatewaySummary.response) return gatewaySummary.response;
  if (
    gatewaySummary.summary &&
    !canReadGatewaySummary(gatewaySummary.summary, input.viewer)
  ) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }
  const gatewayResponse = await proxyMemberGatewayRequest(input.viewer, {
    method: "PATCH",
    adminPath: `/api/internal/admin/members/${encodeURIComponent(input.userId)}`,
    body: JSON.stringify(parsed.value),
  });
  if (gatewayResponse) return gatewayResponse;

  const result = updateMember(input.userId, parsed.value, input.viewer);
  if (!result.ok) {
    return Response.json({ error: result.error }, { status: result.status });
  }
  return Response.json(result.summary, { status: 200 });
}

export async function proxyMemberDeleteRequest(
  input: ProxyMemberDeleteInput,
): Promise<Response> {
  if (!canDeleteMemberManagement(input.viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }
  const gatewayResponse = await proxyMemberGatewayRequest(input.viewer, {
    method: "DELETE",
    adminPath: `/api/internal/admin/members/${encodeURIComponent(input.userId)}`,
  });
  if (gatewayResponse) return gatewayResponse;

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

  const gatewaySummary = await fetchGatewayMemberSummary(
    input.viewer,
    parsed.value.target_user_id,
  );
  if (gatewaySummary.response) return gatewaySummary.response;
  if (
    gatewaySummary.summary &&
    !canReadGatewaySummary(gatewaySummary.summary, input.viewer)
  ) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  const gatewayResponse = await proxyMemberGatewayRequest(input.viewer, {
    method: "POST",
    adminPath: "/api/internal/admin/members/actions",
    body: JSON.stringify(parsed.value),
  });
  if (gatewayResponse) return gatewayResponse;

  if (!findMemberSummary(parsed.value.target_user_id, input.viewer)) {
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

export async function proxyMemberActionListRequest(
  input: ProxyMemberActionListInput,
): Promise<Response> {
  if (!canUseMemberActions(input.viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }

  const query = input.searchParams?.toString();
  const gatewayResponse = await proxyMemberGatewayRequest(input.viewer, {
    method: "GET",
    adminPath: `/api/internal/admin/members/actions${query ? `?${query}` : ""}`,
  });
  if (gatewayResponse) return gatewayResponse;

  return Response.json(
    { actions: [], source: "api-gateway" } satisfies MemberActionListResponse,
    { status: 200 },
  );
}

export async function proxyMemberActionReviewRequest(
  input: ProxyMemberActionReviewInput,
): Promise<Response> {
  if (!canUseMemberActions(input.viewer)) {
    return Response.json({ error: "forbidden" }, { status: 403 });
  }
  const parsed = parseMemberActionReview(input.request);
  if ("error" in parsed) {
    return Response.json({ error: parsed.error }, { status: 400 });
  }
  const gatewayResponse = await proxyMemberGatewayRequest(input.viewer, {
    method: "POST",
    adminPath: `/api/internal/admin/members/actions/${encodeURIComponent(input.actionId)}/reviews`,
    body: JSON.stringify(parsed.value),
  });
  if (gatewayResponse) return gatewayResponse;

  return Response.json({ error: "gateway_unconfigured" }, { status: 500 });
}

function canReadMemberManagement(
  viewer: SessionPayload | null,
): viewer is SessionPayload {
  return (
    viewer?.role === "admin" ||
    viewer?.role === "manager" ||
    viewer?.role === "affiliate"
  );
}

function memberListAdminPath(viewer: SessionPayload): string {
  if (viewer.role !== "affiliate") return "/api/internal/admin/members";
  const params = new URLSearchParams();
  if (viewer.affiliateId) params.set("affiliate_id", viewer.affiliateId);
  const query = params.toString();
  return `/api/internal/admin/members${query ? `?${query}` : ""}`;
}

async function proxyMemberGatewayRequest(
  viewer: SessionPayload,
  input: { method: string; adminPath: string; body?: string },
): Promise<Response | null> {
  const gatewayUrl = process.env.API_GATEWAY_URL;
  const allowMockFallback = allowsMemberMockFallback();
  if (!gatewayUrl) {
    return allowMockFallback
      ? null
      : Response.json({ error: "gateway_unconfigured" }, { status: 500 });
  }

  const body = input.body ?? "";
  const signaturePath = input.adminPath.split("?")[0] ?? input.adminPath;
  const headers = signInternalCall({
    method: input.method,
    path: signaturePath,
    body,
    actorId: viewer.sub,
    role: viewer.role,
  });
  const requestHeaders: Record<string, string> = {
    ...headers,
    "x-request-id": crypto.randomUUID(),
  };
  if (body.length > 0) {
    requestHeaders["content-type"] = "application/json";
  }

  let upstream: Response;
  try {
    upstream = await fetch(new URL(input.adminPath, gatewayUrl), {
      method: input.method,
      headers: requestHeaders,
      body: body.length > 0 ? body : undefined,
      cache: "no-store",
    });
  } catch {
    return allowMockFallback
      ? null
      : Response.json({ error: "gateway_unavailable" }, { status: 502 });
  }
  if (
    (upstream.status === 404 || upstream.status === 501) &&
    allowMockFallback
  ) {
    return null;
  }

  return new Response(await upstream.arrayBuffer(), {
    status: upstream.status,
    headers: copyGatewayHeaders(upstream.headers),
  });
}

async function proxyReferralCaptureRequest(
  viewer: SessionPayload,
  input: { referralCode: string; referredUserId: string },
): Promise<Response | null> {
  const gatewayUrl = process.env.API_GATEWAY_URL;
  if (!gatewayUrl) return null;

  const adminPath = "/api/internal/referrals/capture";
  const body = JSON.stringify({
    referral_code: input.referralCode,
    referred_user_id: input.referredUserId,
  });
  const headers = signInternalCall({
    method: "POST",
    path: adminPath,
    body,
    actorId: viewer.sub,
    role: viewer.role,
  });

  return fetch(new URL(adminPath, gatewayUrl), {
    method: "POST",
    headers: {
      ...headers,
      "x-request-id": crypto.randomUUID(),
      "content-type": "application/json",
    },
    body,
    cache: "no-store",
  }).catch(() => null);
}

async function fetchGatewayMemberSummary(
  viewer: SessionPayload,
  userId: string,
): Promise<{
  summary: MemberManagementSummary | null;
  response: Response | null;
}> {
  const response = await proxyMemberGatewayRequest(viewer, {
    method: "GET",
    adminPath: `/api/internal/admin/members/${encodeURIComponent(userId)}/summary`,
  });
  if (!response) return { summary: null, response: null };
  if (!response.ok) return { summary: null, response };
  return {
    summary: (await response.json()) as MemberManagementSummary,
    response: null,
  };
}

function canReadGatewaySummary(
  summary: MemberManagementSummary,
  viewer: SessionPayload,
): boolean {
  return (
    viewer.role !== "affiliate" ||
    summary.source.affiliate_id === viewer.affiliateId
  );
}

function copyGatewayHeaders(headers: Headers): Headers {
  const next = new Headers();
  const contentType = headers.get("content-type");
  if (contentType) next.set("content-type", contentType);
  return next;
}

function parsePositiveInt(
  value: string | null | undefined,
): number | undefined {
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
    items: body.items.map(
      (item): MemberPendingCase => ({
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
      }),
    ),
    page: body.page,
    page_size: body.page_size,
    total: body.total,
    source: "api-gateway",
  };
}

function canWriteMemberManagement(
  viewer: SessionPayload | null,
): viewer is SessionPayload {
  return (
    viewer?.role === "admin" ||
    viewer?.role === "manager" ||
    viewer?.role === "affiliate"
  );
}

function canDeleteMemberManagement(
  viewer: SessionPayload | null,
): viewer is SessionPayload {
  return viewer?.role === "admin" || viewer?.role === "manager";
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
        typeof candidate.affiliate_id === "string" &&
        candidate.affiliate_id.trim()
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
    candidate.status === "inactive" ||
    candidate.status === "active" ||
    candidate.status === "restricted" ||
    candidate.status === "suspended"
  ) {
    result.status = candidate.status;
  }
  if (
    candidate.risk_level === "low" ||
    candidate.risk_level === "medium" ||
    candidate.risk_level === "high"
  ) {
    result.risk_level = candidate.risk_level;
  }
  if (
    typeof candidate.risk_score === "number" &&
    Number.isFinite(candidate.risk_score)
  ) {
    result.risk_score = candidate.risk_score;
  }
  if (Array.isArray(candidate.restrictions)) {
    result.restrictions = candidate.restrictions
      .filter(
        (restriction): restriction is string => typeof restriction === "string",
      )
      .map((restriction) => restriction.trim())
      .filter((restriction) => restriction.length > 0);
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

function parseMemberActionReview(
  value: unknown,
):
  | { value: { decision: "approve" | "reject"; reason: string } }
  | { error: string } {
  if (!value || typeof value !== "object") {
    return { error: "invalid_request" };
  }
  const candidate = value as Record<string, unknown>;
  if (candidate.decision !== "approve" && candidate.decision !== "reject") {
    return { error: "invalid_decision" };
  }
  if (typeof candidate.reason !== "string" || candidate.reason.trim() === "") {
    return { error: "invalid_reason" };
  }
  return {
    value: {
      decision: candidate.decision,
      reason: candidate.reason.trim(),
    },
  };
}
