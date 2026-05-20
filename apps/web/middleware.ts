import { NextResponse, type NextRequest } from "next/server";
import {
  REFERRAL_ATTRIBUTION_COOKIE,
  REFERRAL_ATTRIBUTION_MAX_AGE_SECONDS,
  referralCodeFromSearchParams,
} from "@/lib/referral-attribution";
import { SESSION_COOKIE_NAME } from "@/lib/auth";
import { recordJourneyRedirect } from "@/lib/telemetry/journey-metrics";

const PROTECTED_PATH_PREFIXES = ["/portfolio", "/topup", "/kyc", "/account"];
const PUBLIC_PATH_PREFIXES = [
  "/",
  "/markets",
  "/legal",
  "/support",
  "/how-it-works",
  "/auth",
];

export function middleware(req: NextRequest) {
  const pathname = req.nextUrl.pathname;
  const response =
    isProtectedPath(pathname) && !req.cookies.get(SESSION_COOKIE_NAME)?.value
      ? redirectToSignIn(req)
      : NextResponse.next();

  setReferralAttributionCookie(req, response);

  return response;
}

function redirectToSignIn(req: NextRequest): NextResponse {
  const redirectUrl = req.nextUrl.clone();
  const returnTo = `${req.nextUrl.pathname}${req.nextUrl.search}`;
  redirectUrl.pathname = "/auth/sign-in";
  redirectUrl.search = "";
  redirectUrl.searchParams.set("return_to", returnTo);
  recordJourneyRedirect(returnTo, "/auth/sign-in");
  return NextResponse.redirect(redirectUrl);
}

function setReferralAttributionCookie(
  req: NextRequest,
  response: NextResponse,
): void {
  const referralCode = referralCodeFromSearchParams(req.nextUrl.searchParams);

  if (referralCode && !req.cookies.get(REFERRAL_ATTRIBUTION_COOKIE)?.value) {
    response.cookies.set({
      name: REFERRAL_ATTRIBUTION_COOKIE,
      value: referralCode,
      httpOnly: true,
      secure: process.env.NODE_ENV === "production",
      sameSite: "lax",
      path: "/",
      maxAge: REFERRAL_ATTRIBUTION_MAX_AGE_SECONDS,
    });
  }
}

function isProtectedPath(pathname: string): boolean {
  if (PUBLIC_PATH_PREFIXES.some((prefix) => pathMatches(pathname, prefix))) {
    return false;
  }
  return PROTECTED_PATH_PREFIXES.some((prefix) =>
    pathMatches(pathname, prefix),
  );
}

function pathMatches(pathname: string, prefix: string): boolean {
  if (prefix === "/") {
    return pathname === "/";
  }
  return pathname === prefix || pathname.startsWith(`${prefix}/`);
}

export const config = {
  matcher: [
    "/((?!_next/static|_next/image|favicon.ico|favicon-32.png|icon-192.png|icon-512.png|apple-touch-icon.png|sw.js).*)",
  ],
};
