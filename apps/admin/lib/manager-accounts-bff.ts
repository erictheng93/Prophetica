import { signInternalCall } from "./internal-call.js";
import type { SessionPayload } from "./session-crypto.js";

export interface RoleAccountSummary {
  user_id: string;
  email: string;
  phone: string;
  role: "manager" | "affiliate" | "admin" | "player" | string;
  affiliate_id: string | null;
  disabled: boolean;
}

export interface ManagerAccountMutationPayload {
  userId?: string;
  displayName?: string;
  email?: string;
  phone?: string;
  password?: string;
  country?: string;
  timezone?: string;
  tier?: string;
  reason: string;
  authorizationBasis: string;
}

interface RoleAccountsResponse {
  accounts: RoleAccountSummary[];
  source: string;
}

export async function fetchManagerAccounts(
  viewer: SessionPayload,
): Promise<RoleAccountSummary[]> {
  if (viewer.role !== "admin") return [];
  const response = await proxyManagerAccountGatewayRequest(viewer, {
    method: "GET",
    adminPath: "/api/internal/admin/role-accounts?role=manager",
  });
  if (!response?.ok) return [];
  const body = (await response.json()) as RoleAccountsResponse;
  return body.accounts;
}

export async function createManagerAccount(
  viewer: SessionPayload,
  payload: ManagerAccountMutationPayload,
): Promise<Response | null> {
  if (viewer.role !== "admin") return null;
  if (
    !payload.userId ||
    !payload.displayName ||
    !payload.email ||
    !payload.phone ||
    !payload.password
  ) {
    return null;
  }
  return proxyManagerAccountGatewayRequest(viewer, {
    method: "POST",
    adminPath: "/api/internal/admin/members",
    body: JSON.stringify({
      user_id: payload.userId,
      display_name: payload.displayName,
      email: payload.email,
      phone: payload.phone,
      line_id: null,
      country: payload.country ?? "TW",
      timezone: payload.timezone ?? "Asia/Taipei",
      tier: payload.tier ?? "standard",
      affiliate_id: null,
      role: "manager",
      password: payload.password,
      reason: payload.reason,
      authorization_basis: payload.authorizationBasis,
    }),
  });
}

export async function updateManagerAccount(
  viewer: SessionPayload,
  userId: string,
  payload: ManagerAccountMutationPayload,
): Promise<Response | null> {
  if (viewer.role !== "admin") return null;
  const body: Record<string, unknown> = {
    reason: payload.reason,
    authorization_basis: payload.authorizationBasis,
  };
  if (payload.displayName) body.display_name = payload.displayName;
  if (payload.email) body.email = payload.email;
  if (payload.phone) body.phone = payload.phone;
  if (payload.password) body.password = payload.password;
  if (payload.country) body.country = payload.country;
  if (payload.timezone) body.timezone = payload.timezone;
  if (payload.tier) body.tier = payload.tier;
  return proxyManagerAccountGatewayRequest(viewer, {
    method: "PATCH",
    adminPath: `/api/internal/admin/members/${encodeURIComponent(userId)}`,
    body: JSON.stringify(body),
  });
}

export async function disableManagerAccount(
  viewer: SessionPayload,
  userId: string,
): Promise<Response | null> {
  if (viewer.role !== "admin") return null;
  return proxyManagerAccountGatewayRequest(viewer, {
    method: "DELETE",
    adminPath: `/api/internal/admin/members/${encodeURIComponent(userId)}`,
  });
}

async function proxyManagerAccountGatewayRequest(
  viewer: SessionPayload,
  input: { method: string; adminPath: string; body?: string },
): Promise<Response | null> {
  const gatewayUrl = process.env.API_GATEWAY_URL;
  if (!gatewayUrl) return null;
  const body = input.body ?? "";
  const signaturePath = input.adminPath.split("?")[0] ?? input.adminPath;
  const headers = signInternalCall({
    method: input.method,
    path: signaturePath,
    body,
    actorId: viewer.sub,
    role: viewer.role,
  });
  return fetch(new URL(input.adminPath, gatewayUrl), {
    method: input.method,
    headers: {
      ...headers,
      "x-request-id": crypto.randomUUID(),
      ...(body ? { "content-type": "application/json" } : {}),
    },
    body: body || undefined,
    cache: "no-store",
  }).catch(() => null);
}
