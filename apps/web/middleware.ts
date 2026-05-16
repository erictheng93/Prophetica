import { NextResponse, type NextRequest } from "next/server";
import {
  REFERRAL_ATTRIBUTION_COOKIE,
  REFERRAL_ATTRIBUTION_MAX_AGE_SECONDS,
  referralCodeFromSearchParams,
} from "@/lib/referral-attribution";

export function middleware(req: NextRequest) {
  const response = NextResponse.next();
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

  return response;
}

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico|favicon-32.png|icon-192.png|icon-512.png|apple-touch-icon.png|sw.js).*)"],
};

