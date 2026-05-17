import { NextResponse } from "next/server";
import { toAdminRole } from "@/lib/admin-auth";
import { getViewer, setSessionCookie } from "@/lib/session";
import {
  getUpstreamSessionToken,
  setUpstreamSessionCookie,
} from "@/lib/upstream-session";

const USER_SERVICE_URL = process.env.USER_SERVICE_URL ?? "http://localhost:8084";

const passkeyActions = {
  "register-start": {
    path: "/v1/auth/webauthn/register/start",
    mode: "start",
  },
  "register-finish": {
    path: "/v1/auth/webauthn/register/finish",
    mode: "finish",
  },
  "assert-start": {
    path: "/v1/auth/webauthn/assert/start",
    mode: "start",
  },
  "assert-finish": {
    path: "/v1/auth/webauthn/assert/finish",
    mode: "finish",
  },
  "revoke": {
    path: "/v1/auth/webauthn/credentials/revoke",
    mode: "finish",
  },
} as const;

interface PasskeyRouteContext {
  params: Promise<{ action: string }>;
}

interface UserServiceLoginResponse {
  user_id?: unknown;
  role?: unknown;
  affiliate_id?: unknown;
  mfa_at_epoch_seconds?: unknown;
  session_token?: unknown;
}

export async function POST(request: Request, context: PasskeyRouteContext) {
  const viewer = await getViewer();
  if (!viewer) {
    return NextResponse.json({ error: "not_authenticated" }, { status: 401 });
  }

  const upstreamSessionToken = await getUpstreamSessionToken();
  if (!upstreamSessionToken) {
    return NextResponse.json({ error: "upstream_session_missing" }, { status: 401 });
  }

  const { action } = await context.params;
  const passkeyAction = passkeyActions[action as keyof typeof passkeyActions];
  if (!passkeyAction) {
    return NextResponse.json({ error: "unknown_passkey_action" }, { status: 404 });
  }

  let body: string | undefined;
  if (passkeyAction.mode === "finish") {
    try {
      const payload = (await request.json()) as Record<string, unknown>;
      body = JSON.stringify({
        ...payload,
        ...(action === "assert-finish" ? { login_surface: "admin_console" } : {}),
      });
    } catch {
      return NextResponse.json({ error: "invalid_json" }, { status: 400 });
    }
  }

  const upstream = await fetch(new URL(passkeyAction.path, USER_SERVICE_URL), {
    method: "POST",
    headers: {
      authorization: `Bearer ${upstreamSessionToken}`,
      ...(body ? { "content-type": "application/json" } : {}),
    },
    ...(body ? { body } : {}),
    cache: "no-store",
  }).catch(() => null);

  if (!upstream) {
    return NextResponse.json({ error: "auth_service_unavailable" }, { status: 503 });
  }

  const responseBody = await upstream.json().catch(() => ({}));

  if (upstream.ok && action === "assert-finish") {
    await refreshAdminStepUpSession(responseBody, viewer);
  }

  return NextResponse.json(responseBody, { status: upstream.status });
}

async function refreshAdminStepUpSession(
  responseBody: unknown,
  fallback: { sub: string; role: "admin" | "manager" | "affiliate"; affiliateId?: string },
) {
  if (!responseBody || typeof responseBody !== "object") return;
  const response = responseBody as UserServiceLoginResponse;
  if (typeof response.session_token !== "string") return;

  await setUpstreamSessionCookie(response.session_token);
  const role = toAdminRole(response.role) ?? fallback.role;
  const sub = typeof response.user_id === "string" ? response.user_id : fallback.sub;
  const affiliateId =
    typeof response.affiliate_id === "string" ? response.affiliate_id : fallback.affiliateId;
  const mfaAt =
    typeof response.mfa_at_epoch_seconds === "number" ? response.mfa_at_epoch_seconds : undefined;

  await setSessionCookie({
    sub,
    role,
    ...(affiliateId ? { affiliateId } : {}),
    ...(mfaAt ? { mfaAt, amr: ["pwd", "hwk"] } : {}),
  });
}
