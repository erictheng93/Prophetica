import "server-only";
import { signInternalCall } from "./internal-call.js";
import type {
  CompensationProposalCreateRequest,
  CompensationProposerKind,
  CompensationProposalStatus,
  CompensationReviewDecision,
} from "./compensation.js";
import type { SessionPayload } from "./session-crypto.js";

type FetchLike = (
  input: RequestInfo | URL,
  init?: RequestInit,
) => Promise<Response>;

export interface CompensationBffDeps {
  gatewayUrl?: string;
  secret?: string;
  fetchImpl?: FetchLike;
}

export interface CompensationProposalListInput {
  status?: CompensationProposalStatus;
  proposerKind?: CompensationProposerKind;
  page?: number;
  pageSize?: number;
}

export interface CompensationProposalCreateInput
  extends CompensationProposalCreateRequest {
  userId: string;
}

export async function listCompensationProposals(
  viewer: SessionPayload,
  input: CompensationProposalListInput = {},
  deps: CompensationBffDeps = {},
): Promise<Response> {
  const params = new URLSearchParams();
  if (input.status) params.set("status", input.status);
  if (input.proposerKind) params.set("proposer_kind", input.proposerKind);
  if (input.page !== undefined) params.set("page", String(input.page));
  if (input.pageSize !== undefined) params.set("page_size", String(input.pageSize));
  const query = params.toString();
  return proxyCompensationGatewayRequest(viewer, {
    method: "GET",
    adminPath: `/api/internal/admin/compensation-proposals${query ? `?${query}` : ""}`,
  }, deps);
}

export async function createCompensationProposal(
  viewer: SessionPayload,
  input: CompensationProposalCreateInput,
  deps: CompensationBffDeps = {},
): Promise<Response> {
  const { userId, ...request } = input;
  return proxyCompensationGatewayRequest(viewer, {
    method: "POST",
    adminPath: `/api/internal/admin/members/${encodeURIComponent(userId)}/compensation-proposals`,
    body: JSON.stringify(request),
  }, deps);
}

export async function reviewCompensationProposal(
  viewer: SessionPayload,
  proposalId: string,
  decision: CompensationReviewDecision,
  note?: string,
  deps: CompensationBffDeps = {},
): Promise<Response> {
  return proxyCompensationGatewayRequest(viewer, {
    method: "POST",
    adminPath: `/api/internal/admin/compensation-proposals/${encodeURIComponent(proposalId)}/review`,
    body: JSON.stringify({ decision, note }),
  }, deps);
}

export async function retryPostingForProposal(
  viewer: SessionPayload,
  proposalId: string,
  deps: CompensationBffDeps = {},
): Promise<Response> {
  return proxyCompensationGatewayRequest(viewer, {
    method: "POST",
    adminPath: `/api/internal/admin/compensation-proposals/${encodeURIComponent(proposalId)}/retry-posting`,
    body: "{}",
  }, deps);
}

async function proxyCompensationGatewayRequest(
  viewer: SessionPayload,
  input: { method: "GET" | "POST"; adminPath: string; body?: string },
  deps: CompensationBffDeps,
): Promise<Response> {
  const gatewayUrl = deps.gatewayUrl ?? process.env.API_GATEWAY_URL;
  if (!gatewayUrl) {
    return Response.json({ error: "gateway_unconfigured" }, { status: 500 });
  }

  const body = input.body ?? "";
  const signaturePath = input.adminPath.split("?")[0] ?? input.adminPath;
  const headers = signInternalCall({
    method: input.method,
    path: signaturePath,
    body,
    actorId: viewer.sub,
    role: viewer.role,
    secret: deps.secret,
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
    upstream = await (deps.fetchImpl ?? fetch)(
      new URL(input.adminPath, gatewayUrl),
      {
        method: input.method,
        headers: requestHeaders,
        body: body.length > 0 ? body : undefined,
        cache: "no-store",
      },
    );
  } catch {
    return Response.json({ error: "gateway_unavailable" }, { status: 502 });
  }

  return new Response(await upstream.arrayBuffer(), {
    status: upstream.status,
    headers: copyGatewayHeaders(upstream.headers),
  });
}

function copyGatewayHeaders(headers: Headers): Headers {
  const next = new Headers();
  const contentType = headers.get("content-type");
  if (contentType) next.set("content-type", contentType);
  return next;
}
