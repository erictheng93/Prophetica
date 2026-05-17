import { NextResponse } from "next/server";
import { toAdminRole } from "@/lib/admin-auth";
import { defaultLandingFor, type Role } from "@/lib/rbac";
import { setSessionCookie } from "@/lib/session";
import { setUpstreamSessionCookie } from "@/lib/upstream-session";

interface LoginBody {
  email?: unknown;
  password?: unknown;
  mfaCode?: unknown;
  mfaRecoveryCode?: unknown;
}

interface UserServiceLoginResponse {
  user_id?: unknown;
  role?: unknown;
  affiliate_id?: unknown;
  session_token?: unknown;
}

const USER_SERVICE_URL = process.env.USER_SERVICE_URL ?? "http://localhost:8084";

export async function POST(request: Request) {
  let body: LoginBody;
  try {
    body = (await request.json()) as LoginBody;
  } catch {
    return NextResponse.json({ error: "invalid_json" }, { status: 400 });
  }
  const email = typeof body.email === "string" ? body.email : "";
  const password = typeof body.password === "string" ? body.password : "";
  const mfaCode = typeof body.mfaCode === "string" ? body.mfaCode.trim() : "";
  const mfaRecoveryCode =
    typeof body.mfaRecoveryCode === "string" ? body.mfaRecoveryCode.trim() : "";

  if (!email || !password) {
    return NextResponse.json({ error: "missing_credentials" }, { status: 400 });
  }

  const upstream = await fetch(new URL("/api/auth/login", USER_SERVICE_URL), {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      identifier: email,
      identifier_kind: "email",
      password,
      remember: false,
      ...(mfaCode ? { mfa_code: mfaCode } : {}),
      ...(mfaRecoveryCode ? { mfa_recovery_code: mfaRecoveryCode } : {}),
    }),
    cache: "no-store",
  }).catch(() => null);

  if (!upstream) {
    return NextResponse.json({ error: "auth_service_unavailable" }, { status: 503 });
  }

  if (upstream.status === 401) {
    return NextResponse.json({ error: "invalid_credentials" }, { status: 401 });
  }

  if (!upstream.ok) {
    return NextResponse.json({ error: "auth_service_error" }, { status: upstream.status });
  }

  let user: UserServiceLoginResponse;
  try {
    user = (await upstream.json()) as UserServiceLoginResponse;
  } catch {
    return NextResponse.json({ error: "invalid_auth_response" }, { status: 502 });
  }

  const sub = typeof user.user_id === "string" ? user.user_id : "";
  const role = toAdminRole(user.role);
  const affiliateId = typeof user.affiliate_id === "string" ? user.affiliate_id : undefined;

  if (!sub || !role || typeof user.session_token !== "string") {
    return NextResponse.json({ error: "admin_access_denied" }, { status: 403 });
  }

  await setSessionCookie({
    sub,
    role,
    ...(affiliateId ? { affiliateId } : {}),
  });
  await setUpstreamSessionCookie(user.session_token);

  return NextResponse.json({
    role,
    redirect: defaultLandingFor(role),
  });
}
