import "server-only";
import { cookies } from "next/headers";
import {
  resolveSessionSecret,
  signSession,
  verifyAdminSessionToken,
  verifySession as verifySessionPure,
  type SessionPayload,
} from "./session-crypto";

/*
 * Production admin sessions are user-service Ed25519 compact JWS tokens.
 * The legacy HMAC envelope remains available only for local/E2E fixtures; it
 * is rejected when NODE_ENV=production by session-crypto.
 */

export const SESSION_COOKIE_NAME = "proph_admin_session";

export type { SessionPayload } from "./session-crypto";

function getSecret(): string {
  return resolveSessionSecret();
}

export async function verifySession(token: string): Promise<SessionPayload | null> {
  return verifyAdminSessionToken(token);
}

export async function getViewer(): Promise<SessionPayload | null> {
  const token = (await cookies()).get(SESSION_COOKIE_NAME)?.value;
  if (!token) return null;
  return verifySession(token);
}

export async function setSessionCookie(payload: Omit<SessionPayload, "iat" | "exp">): Promise<void> {
  const token = await signSession(payload, getSecret());
  await setUserServiceSessionCookie(token);
}

export async function setUserServiceSessionCookie(token: string): Promise<void> {
  (await cookies()).set({
    name: SESSION_COOKIE_NAME,
    value: token,
    httpOnly: true,
    secure: process.env.NODE_ENV === "production",
    sameSite: "strict",
    path: "/",
    maxAge: 60 * 60 * 8,
  });
}

export async function clearSessionCookie(): Promise<void> {
  (await cookies()).set({
    name: SESSION_COOKIE_NAME,
    value: "",
    httpOnly: true,
    secure: process.env.NODE_ENV === "production",
    sameSite: "strict",
    path: "/",
    maxAge: 0,
  });
}
