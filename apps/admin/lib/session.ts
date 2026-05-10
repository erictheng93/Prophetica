import "server-only";
import { cookies } from "next/headers";
import { signSession, verifySession as verifySessionPure, type SessionPayload } from "./session-crypto";

/*
 * SPEC-017 §6.3 amendment — Phase 2 mock session.
 *
 * Why HMAC-SHA256 here, not Ed25519 JWT yet
 *   The real signer is user-service (Rust) and is part of Phase 3 work
 *   (see TODO.md LG-7). For Phase 2 we only need a tamper-evident envelope
 *   so browsers can't trivially forge a `role=admin` cookie. Web Crypto's
 *   HMAC-SHA256 is enough; rotating in Ed25519 later is a swap of `verify`
 *   inside session-crypto without touching call sites here.
 *
 * The signing key is read from `ADMIN_SESSION_SECRET`. In production this
 * comes from Vault (SPEC-010 §3.3); locally it falls back to a clearly-
 * marked dev string so engineers can iterate without bootstrapping Vault.
 * Phase 3 will fail-closed on missing secret.
 */

export const SESSION_COOKIE_NAME = "proph_admin_session";
const DEV_SECRET = "dev-only-admin-session-secret-do-not-use-in-prod";

export type { SessionPayload } from "./session-crypto";

function getSecret(): string {
  return process.env.ADMIN_SESSION_SECRET ?? DEV_SECRET;
}

export async function verifySession(token: string): Promise<SessionPayload | null> {
  return verifySessionPure(token, getSecret());
}

export async function getViewer(): Promise<SessionPayload | null> {
  const token = (await cookies()).get(SESSION_COOKIE_NAME)?.value;
  if (!token) return null;
  return verifySession(token);
}

export async function setSessionCookie(payload: Omit<SessionPayload, "iat" | "exp">): Promise<void> {
  const token = await signSession(payload, getSecret());
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
