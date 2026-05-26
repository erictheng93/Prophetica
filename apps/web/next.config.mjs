import createNextIntlPlugin from "next-intl/plugin";

const withNextIntl = createNextIntlPlugin("./i18n/request.ts");

// SPEC-005a §8 — CSP for KYC routes only. The real KYC provider SDK is not
// enabled yet, so this route keeps a tight provider-neutral baseline and
// avoids blessing any retired vendor origins.
//
// Scope: /kyc/:path* only. Site-wide CSP is the intended Phase 2 once
// SPEC-010 (Sift / Sardine fingerprint SDKs) lands and we can negotiate a
// single global directive set; see SPEC-005a §11.4.
//
// Mock provider does not need external allowances; dev / test must keep
// passing under script-src 'self'.
const KYC_CSP_HEADER_VALUE = [
  "default-src 'self'",
  "script-src 'self'",
  "style-src 'self' 'unsafe-inline'",
  "connect-src 'self'",
  "frame-src 'self'",
  "img-src 'self' data: blob:",
  "media-src 'self' blob:",
  "worker-src 'self' blob:",
].join("; ");

const KYC_ROUTE_GLOB = "/kyc/:path*";

// SPEC-005a §9.2 — runtime: "nodejs" fallback for /kyc/tier-2 and /kyc/tier-3
// is reactive, not preemptive. Add `export const runtime = "nodejs"` to the
// route-level page only if live provider validation proves the browser SDK
// breaks Cloudflare Pages Edge runtime. Mock provider is pure JS so today both
// routes are Edge-safe.

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  turbopack: {
    resolveAlias: {
      "./idempotency.js": "./lib/idempotency.ts",
      "./kyc-idempotency.js": "./lib/kyc-idempotency.ts",
      "./kyc-metrics.js": "./lib/telemetry/kyc-metrics.ts",
      "./kyc-state-machine.js": "./lib/kyc-state-machine.ts",
      "./latency.js": "./lib/observability/latency.ts",
      "./logger.js": "./lib/observability/logger.ts",
      "./market-api.js": "./lib/market-api.ts",
      "./mock-markets.js": "./lib/mock-markets.ts",
      "./mock-portfolio.js": "./lib/mock-portfolio.ts",
      "./observability/trace.js": "./lib/observability/trace.ts",
      "./prom-registry.js": "./lib/telemetry/prom-registry.ts",
      "./trace.js": "./lib/observability/trace.ts",
      "./vitals.js": "./lib/observability/vitals.ts",
    },
  },
  transpilePackages: [
    "@prophetica/ui",
    "@prophetica/design-tokens",
    "@prophetica/ts-sdk",
  ],
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
