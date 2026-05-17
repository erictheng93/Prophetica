import { NextResponse } from "next/server";

interface LiffSessionBody {
  id_token?: unknown;
}

interface UserServiceLiffSessionResponse {
  user_id?: unknown;
  session_token?: unknown;
  expires_at_epoch_seconds?: unknown;
}

const USER_SERVICE_URL = process.env.USER_SERVICE_URL ?? "http://localhost:8084";
const UPSTREAM_SESSION_COOKIE_NAME = "proph_user_service_session";

export async function POST(request: Request) {
  let body: LiffSessionBody;
  try {
    body = (await request.json()) as LiffSessionBody;
  } catch {
    return NextResponse.json({ error: "invalid_json" }, { status: 400 });
  }

  const idToken = typeof body.id_token === "string" ? body.id_token.trim() : "";
  if (!idToken) {
    return NextResponse.json({ error: "missing_id_token" }, { status: 400 });
  }

  const upstream = await fetch(
    new URL("/v1/auth/liff/session", USER_SERVICE_URL),
    {
      method: "POST",
      headers: liffForwardHeaders(request),
      body: JSON.stringify({
        id_token: idToken,
        remember: true,
      }),
      cache: "no-store",
    },
  ).catch(() => null);

  if (!upstream) {
    return NextResponse.json({ error: "auth_service_unavailable" }, { status: 503 });
  }
  if (upstream.status === 401 || upstream.status === 403) {
    return NextResponse.json({ error: "invalid_liff_identity" }, { status: 401 });
  }
  if (!upstream.ok) {
    return NextResponse.json({ error: "auth_service_error" }, { status: upstream.status });
  }

  let payload: UserServiceLiffSessionResponse;
  try {
    payload = (await upstream.json()) as UserServiceLiffSessionResponse;
  } catch {
    return NextResponse.json({ error: "invalid_auth_response" }, { status: 502 });
  }

  const userId = typeof payload.user_id === "string" ? payload.user_id : "";
  const sessionToken =
    typeof payload.session_token === "string" ? payload.session_token : "";
  const expiresAt =
    typeof payload.expires_at_epoch_seconds === "number"
      ? payload.expires_at_epoch_seconds
      : 0;

  if (!userId || !sessionToken || expiresAt <= 0) {
    return NextResponse.json({ error: "invalid_auth_response" }, { status: 502 });
  }

  const now = Math.floor(Date.now() / 1000);
  const maxAge = Math.max(0, expiresAt - now);
  const response = NextResponse.json({ user_id: userId });
  response.cookies.set(UPSTREAM_SESSION_COOKIE_NAME, sessionToken, {
    httpOnly: true,
    sameSite: "strict",
    secure: process.env.NODE_ENV === "production",
    path: "/",
    maxAge,
  });

  return response;
}

function liffForwardHeaders(request: Request): Headers {
  const headers = new Headers({ "content-type": "application/json" });
  for (const name of [
    "user-agent",
    "cf-connecting-ip",
    "cf-ipcountry",
    "cf-region",
    "cf-ipcity",
    "cf-asn",
    "x-forwarded-for",
    "x-real-ip",
    "x-request-id",
    "x-correlation-id",
  ]) {
    const value = request.headers.get(name);
    if (value) headers.set(name, value);
  }
  return headers;
}
