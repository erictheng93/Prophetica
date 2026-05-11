import { NextResponse, type NextRequest } from "next/server";
import { canAccess, defaultLandingFor } from "@/lib/rbac";
import { verifySession } from "@/lib/session-crypto";

const SESSION_COOKIE_NAME = "proph_admin_session";
const DEV_SECRET = "dev-only-admin-session-secret-do-not-use-in-prod";

function getSecret(): string {
  return process.env.ADMIN_SESSION_SECRET ?? DEV_SECRET;
}

/*
 * SPEC-017 §6.3 (amended 2026-05-10) — admin middleware enforces:
 *   1. Cloudflare Access / mTLS gate (placeholder — Phase 3 will read the
 *      edge-supplied attestation header for real verification)
 *   2. Application-layer session: every non-public route requires a valid
 *      session cookie, and the cookie's role must satisfy lib/rbac
 *
 * Public paths that bypass auth entirely:
 *   /login, /api/auth/*, /api/health, /_next, /favicon, /robots.txt
 */

const PUBLIC_PREFIXES = [
  "/login",
  "/api/auth",
  "/api/health",
  "/_next",
  "/favicon",
  "/favicon.ico",
  "/robots.txt",
  "/manifest.webmanifest",
  "/favicon-32.png",
  "/icon-192.png",
  "/icon-512.png",
  "/apple-touch-icon.png",
  "/sw.js",
  "/offline",
];

function isPublic(pathname: string): boolean {
  return PUBLIC_PREFIXES.some(
    (prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`),
  );
}

export async function middleware(req: NextRequest) {
  const { pathname } = req.nextUrl;

  if (isPublic(pathname)) {
    return NextResponse.next();
  }

  // Phase 3 — replace with real Cf-Mtls-Verified header check. Today this
  // only fail-closes in production when the placeholder header is missing.
  if (
    process.env.NODE_ENV === "production" &&
    !req.headers.get("x-internal-mtls")
  ) {
    return new NextResponse("Forbidden", {
      status: 403,
      headers: { "content-type": "text/plain; charset=utf-8" },
    });
  }

  const token = req.cookies.get(SESSION_COOKIE_NAME)?.value;
  const session = token ? await verifySession(token, getSecret()) : null;

  if (!session) {
    const loginUrl = req.nextUrl.clone();
    loginUrl.pathname = "/login";
    if (pathname !== "/") {
      loginUrl.searchParams.set("from", pathname);
    } else {
      loginUrl.searchParams.delete("from");
    }
    return NextResponse.redirect(loginUrl);
  }

  if (!canAccess(pathname, session.role)) {
    // Send affiliates landing on internal routes back to their portal so they
    // never see the bare 403; everyone else gets the explicit 403 page.
    // (Use a path-segment check, not startsWith — /members starts with /me.)
    const isAffiliateOwnPath =
      pathname === "/me/affiliate" || pathname.startsWith("/me/affiliate/");
    const target = req.nextUrl.clone();
    target.pathname =
      session.role === "affiliate" && !isAffiliateOwnPath
        ? defaultLandingFor("affiliate")
        : "/403";
    target.search = "";
    return NextResponse.redirect(target);
  }

  return NextResponse.next();
}

export const config = {
  matcher: ["/((?!_next/static|_next/image).*)"],
};
