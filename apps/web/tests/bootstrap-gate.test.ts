import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const readWebFile = (path: string) =>
  readFileSync(join(process.cwd(), path), "utf8");

test("Cloudflare worker adapter is wired through OpenNext and Wrangler", () => {
  const nextConfig = readWebFile("next.config.mjs");
  const openNextConfig = readWebFile("open-next.config.ts");
  const packageJson = JSON.parse(readWebFile("package.json")) as {
    dependencies?: Record<string, string>;
  };
  const wranglerConfig = readWebFile("wrangler.toml");

  assert.doesNotMatch(nextConfig, /@cloudflare\/next-on-pages/);
  assert.match(openNextConfig, /\bdefineCloudflareConfig\b/);
  assert.ok(packageJson.dependencies?.["@opennextjs/cloudflare"]);
  assert.equal(
    packageJson.dependencies?.["@cloudflare/next-on-pages"],
    undefined,
  );
  assert.match(wranglerConfig, /^main = ["']\.open-next\/worker\.js["']$/m);
  assert.match(
    wranglerConfig,
    /^compatibility_flags = \["nodejs_compat", "global_fetch_strictly_public"\]$/m,
  );
  assert.match(wranglerConfig, /^\[assets\]$/m);
  assert.match(wranglerConfig, /^directory = ["']\.open-next\/assets["']$/m);
  assert.match(wranglerConfig, /^binding = ["']WORKER_SELF_REFERENCE["']$/m);
});

test("health route remains static and returns the SPEC-017 ok payload", () => {
  const source = readWebFile("app/api/health/route.ts");

  assert.match(source, /^export const dynamic = ["']force-static["'];?$/m);
  assert.match(source, /Response\.json\(\{\s*status:\s*["']ok["']/);
});

test("landing page retains the SPEC-017 bootstrap marker for acceptance checks", () => {
  const source = readWebFile("app/page.tsx");

  assert.match(source, /prophetica web - bootstrapping/);
});
