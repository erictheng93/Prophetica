import path from "node:path";
import { fileURLToPath } from "node:url";
import createNextIntlPlugin from "next-intl/plugin";

const appDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(appDir, "../..");

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  turbopack: {
    root: repoRoot,
    resolveAlias: {
      "./client.js": "../../packages/ts-sdk/src/analytics/client.ts",
      "./events.js": "../../packages/ts-sdk/src/analytics/events.ts",
    },
  },
  transpilePackages: [
    "@prophetica/ui",
    "@prophetica/design-tokens",
    "@prophetica/ts-sdk",
  ],
  webpack(config) {
    config.resolve.extensionAlias = {
      ...config.resolve.extensionAlias,
      ".js": [".ts", ".tsx", ".js"],
      ".mjs": [".mts", ".mjs"],
    };
    return config;
  },
};

const withNextIntl = createNextIntlPlugin();

export default withNextIntl(nextConfig);
