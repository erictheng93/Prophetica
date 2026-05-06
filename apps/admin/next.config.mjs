/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  // Workspace packages need to be transpiled because they ship as TS.
  transpilePackages: ["@prophetica/ui", "@prophetica/design-tokens"],
};

export default nextConfig;
