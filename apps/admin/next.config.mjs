/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
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
