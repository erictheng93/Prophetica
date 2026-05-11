import createNextIntlPlugin from "next-intl/plugin";

const withNextIntl = createNextIntlPlugin("./i18n/request.ts");

// SPEC-005a §8 — CSP for KYC routes only. SumSub Web SDK loads its bundle
// from static.sumsub.com, opens an iframe to *.sumsub.com, talks to
// api.sumsub.com over HTTPS + WSS, and uses blob: workers for image
// processing. 'unsafe-inline' is required by SumSub for cross-frame messaging
// (matches their published integration guide).
//
// Scope: /kyc/:path* only. Site-wide CSP is the intended Phase 2 once
// SPEC-010 (Sift / Sardine fingerprint SDKs) lands and we can negotiate a
// single global directive set; see SPEC-005a §11.4.
//
// Mock provider does NOT need any of these allowances; dev / test must keep
// passing under script-src 'self'.
const KYC_CSP_HEADER_VALUE = [
  "default-src 'self'",
  "script-src 'self' 'unsafe-inline' https://static.sumsub.com",
  "style-src 'self' 'unsafe-inline'",
  "connect-src 'self' https://api.sumsub.com https://*.sumsub.com wss://*.sumsub.com",
  "frame-src 'self' https://*.sumsub.com",
  "img-src 'self' data: blob: https://*.sumsub.com",
  "media-src 'self' blob:",
  "worker-src 'self' blob:",
].join("; ");

const KYC_ROUTE_GLOB = "/kyc/:path*";

// SPEC-005a §9.2 — runtime: "nodejs" fallback for /kyc/tier-2 and /kyc/tier-3
// is reactive, not preemptive. Add `export const runtime = "nodejs"` to the
// route-level page only if PoC-B §4.2.1 confirms the SumSub dynamic import
// breaks Cloudflare Pages Edge runtime. Mock provider is pure JS so today
// both routes are Edge-safe.

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  transpilePackages: ["@prophetica/ui", "@prophetica/design-tokens"],
  async headers() {
    return [
      {
        source: KYC_ROUTE_GLOB,
        headers: [
          { key: "Content-Security-Policy", value: KYC_CSP_HEADER_VALUE },
        ],
      },
    ];
  },
  webpack(config) {
    config.resolve.extensionAlias = {
      ...config.resolve.extensionAlias,
      ".js": [".ts", ".tsx", ".js"],
      ".mjs": [".mts", ".mjs"],
    };
    return config;
  },
};

export default withNextIntl(nextConfig);
