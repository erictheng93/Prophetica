import { cookies } from "next/headers";

import {
  createSessionToken,
  SESSION_COOKIE_NAME,
  verifySessionToken,
} from "./auth";

const SESSION_TTL_SECONDS = 60 * 60 * 24;
const REMEMBERED_SESSION_TTL_SECONDS = 60 * 60 * 24 * 14;

export interface Viewer {
  userId: string;
}

export interface SetViewerSessionInput {
  userId: string;
  remember: boolean;
  upstreamSessionToken?: string;
}

export async function getViewer(): Promise<Viewer | null> {
  const cookieStore = await cookies();
  const token = cookieStore.get(SESSION_COOKIE_NAME)?.value;

  if (!token) {
    return null;
  }

  const payload = await verifySessionToken(token);
  return payload ? { userId: payload.userId } : null;
}

export async function setViewerSession({
  userId,
  remember,
  upstreamSessionToken,
}: SetViewerSessionInput): Promise<void> {
  const now = Math.floor(Date.now() / 1000);
  const ttl = remember ? REMEMBERED_SESSION_TTL_SECONDS : SESSION_TTL_SECONDS;
  const token = await createSessionToken({
    userId,
    issuedAtEpochSeconds: now,
    expiresAtEpochSeconds: now + ttl,
  });
  const cookieStore = await cookies();

  cookieStore.set(SESSION_COOKIE_NAME, token, {
    httpOnly: true,
    sameSite: "lax",
    secure: process.env.NODE_ENV === "production",
    path: "/",
    maxAge: ttl,
  });
  if (upstreamSessionToken) {
    cookieStore.set("proph_user_service_session", upstreamSessionToken, {
      httpOnly: true,
      sameSite: "strict",
      secure: process.env.NODE_ENV === "production",
      path: "/",
      maxAge: ttl,
    });
  }
}

export async function getUpstreamSessionToken(): Promise<string | null> {
  const cookieStore = await cookies();
  return cookieStore.get("proph_user_service_session")?.value ?? null;
}
