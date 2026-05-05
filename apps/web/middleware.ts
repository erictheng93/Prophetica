import { type NextRequest, NextResponse } from "next/server";

/*
 * Geo-block middleware — SPEC-001 §2 Layer 1 / Layer 2 (user-facing redirect).
 *
 * Cloudflare WAF (real Layer 1) blocks 🔴 traffic at the edge BEFORE this
 * runs. This middleware is the application-level safety net: it catches
 * requests that slipped past WAF (mis-routed deployments, dev environments,
 * future misconfiguration) and redirects to /legal/geo-block.
 *
 * Implementation status (2026-05-05): SCAFFOLD ONLY.
 *   - Real geo-resolution requires the dual-source verification stack
 *     (MaxMind GeoIP2 + IP2Location, SPEC-001 §2.1 Layer 2). The lookup
 *     client lives in `packages/ts-sdk/geo` (pending — Codex backend exposes
 *     it once Cargo workspace is online).
 *   - 🔴 country list is intentionally NOT hard-coded here: SPEC-000 §1 marks
 *     it as pending final legal opinion. Hard-coding pre-opinion creates
 *     audit-trail risk.
 *   - This file currently passes all requests through. The redirect
 *     mechanism, exemption list, and headers are wired so flipping the
 *     resolver on later is a one-line change.
 *
 * Compliance contract:
 *   - The `/legal/geo-block` route MUST be reachable from blocked geographies
 *     (it is the destination, not a victim).
 *   - Static assets (_next/static, favicon, etc.) are exempted to avoid
 *     trapping resources behind the gate.
 *   - VPN-bypass logic is intentionally absent (SPEC-001 §2.2).
 */

const GEO_BLOCK_PATH = "/legal/geo-block";

// Routes that must remain reachable from any geography. Keep this list tiny.
const EXEMPT_PREFIXES = [
  GEO_BLOCK_PATH,
  "/api/health",
  "/_next",
  "/favicon",
  "/robots.txt",
  "/sitemap.xml",
];

function isExempt(pathname: string): boolean {
  return EXEMPT_PREFIXES.some((prefix) => pathname.startsWith(prefix));
}

/**
 * Resolve the requesting country code from the inbound request.
 *
 * Production:   Cloudflare injects `cf-ipcountry` on every Worker / Pages
 *               request. When deployed behind Cloudflare this is the source
 *               of truth and is attested by Cloudflare's edge.
 * Development:  header is absent; resolver returns null and the gate is open.
 *
 * Returning null is treated as "unknown" — we do NOT fail-closed at the
 * middleware layer because Cloudflare WAF (Layer 1) already enforced the
 * primary block. Fail-closed here would lock out local dev entirely.
 */
function resolveCountry(req: NextRequest): string | null {
  const cfCountry = req.headers.get("cf-ipcountry");
  return cfCountry ? cfCountry.toUpperCase() : null;
}

/**
 * Returns true if the country is on the 🔴 block list.
 *
 * Currently always returns false — the real list is pending legal opinion
 * (SPEC-000 §1). When the list is finalized, replace this body with a lookup
 * against `packages/ts-sdk/geo` constants. Keep the function signature
 * stable so the policy update is a single-file diff.
 */
function isBlockedCountry(_country: string): boolean {
  return false;
}

export function middleware(req: NextRequest) {
  const { pathname } = req.nextUrl;
  if (isExempt(pathname)) return NextResponse.next();

  const country = resolveCountry(req);
  if (country && isBlockedCountry(country)) {
    const url = req.nextUrl.clone();
    url.pathname = GEO_BLOCK_PATH;
    return NextResponse.redirect(url);
  }

  return NextResponse.next();
}

export const config = {
  // Run on every request except static assets — narrow scope keeps the
  // edge-runtime budget small and avoids touching Next.js internals.
  matcher: ["/((?!_next/static|_next/image).*)"],
};
