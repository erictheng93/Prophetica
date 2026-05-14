const DEFAULT_ALLOWED_DEV_ORIGINS = ["192.168.68.93"];

function normalizeAllowedDevOrigin(value) {
  const trimmed = value.trim();
  if (!trimmed) return null;

  if (trimmed.includes("://")) {
    try {
      return new URL(trimmed).hostname.toLowerCase();
    } catch {
      return trimmed.toLowerCase();
    }
  }

  return trimmed.replace(/:\d+$/, "").toLowerCase();
}

export function parseAllowedDevOrigins(value = process.env.NEXT_ALLOWED_DEV_ORIGINS) {
  const configuredOrigins = (value ?? "")
    .split(",")
    .map(normalizeAllowedDevOrigin)
    .filter(Boolean);

  return Array.from(new Set([...DEFAULT_ALLOWED_DEV_ORIGINS, ...configuredOrigins]));
}

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  allowedDevOrigins: parseAllowedDevOrigins(),
  // Workspace packages need to be transpiled because they ship as TS.
  transpilePackages: [
    "@prophetica/ui",
    "@prophetica/design-tokens",
    "@prophetica/ts-sdk",
  ],
  // lib/* uses NodeNext-style `./foo.js` extensions so node:test (tsc + node)
  // can run them. Tell webpack to strip `.js` when looking up TS sources so
  // the dev server and prod build still resolve them.
  webpack: (config) => {
    config.resolve = config.resolve ?? {};
    config.resolve.extensionAlias = {
      ...(config.resolve.extensionAlias ?? {}),
      ".js": [".ts", ".tsx", ".js"],
    };
    return config;
  },
};

export default nextConfig;
