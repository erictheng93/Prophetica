import { NextResponse, type NextRequest } from "next/server";

/*
 * SPEC-017 §6.3 admin middleware — placeholder for the mTLS / internal-network
 * gate that will eventually live at the ingress layer.
 *
 * What ships in production
 *   - Cloudflare Access (SSO + device posture) in front of the deployment
 *   - mTLS between Access and the origin so this middleware can trust the
 *     `Cf-Mtls-Verified` (or equivalent) header
 *   - IP allowlist as a defence-in-depth layer
 *
 * What this file does today
 *   - Reads the placeholder `x-internal-mtls` header. If absent, returns 403
 *     with copy that does not reveal what's missing.
 *   - Exempts /api/health so liveness probes from infra still work.
 *   - Does NOT do any real verification — placeholder only. Replace with
 *     edge-supplied attestation once SPEC-010 §3.3 lands.
 */

const EXEMPT_PREFIXES = ["/api/health", "/_next", "/favicon", "/robots.txt"];

export function middleware(req: NextRequest) {
  const { pathname } = req.nextUrl;
  if (EXEMPT_PREFIXES.some((p) => pathname.startsWith(p))) {
    return NextResponse.next();
  }

  // Local dev escape hatch: when running outside an Access gateway the
  // header is absent. Fail open in dev so engineers can iterate; real
  // deployments MUST set this to fail-closed.
  if (process.env.NODE_ENV !== "production") {
    return NextResponse.next();
  }

  const mtlsHeader = req.headers.get("x-internal-mtls");
  if (!mtlsHeader) {
    return new NextResponse("Forbidden", {
      status: 403,
      headers: { "content-type": "text/plain; charset=utf-8" },
    });
  }

  return NextResponse.next();
}

export const config = {
  matcher: ["/((?!_next/static|_next/image).*)"],
};
