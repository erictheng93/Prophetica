import "server-only";
import { cookies } from "next/headers";

export const UPSTREAM_SESSION_COOKIE_NAME = "proph_user_service_session";

export async function getUpstreamSessionToken(): Promise<string | null> {
  return (await cookies()).get(UPSTREAM_SESSION_COOKIE_NAME)?.value ?? null;
}

export async function setUpstreamSessionCookie(token: string): Promise<void> {
  (await cookies()).set({
    name: UPSTREAM_SESSION_COOKIE_NAME,
    value: token,
    httpOnly: true,
    secure: process.env.NODE_ENV === "production",
    sameSite: "strict",
    path: "/",
    maxAge: 60 * 60 * 8,
  });
}

export async function clearUpstreamSessionCookie(): Promise<void> {
  (await cookies()).set({
    name: UPSTREAM_SESSION_COOKIE_NAME,
    value: "",
    httpOnly: true,
    secure: process.env.NODE_ENV === "production",
    sameSite: "strict",
    path: "/",
    maxAge: 0,
  });
}
