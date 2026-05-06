import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const readWebFile = (path: string) => readFileSync(join(process.cwd(), path), "utf8");

test("next config wires Cloudflare next-on-pages development platform support", () => {
  const source = readWebFile("next.config.mjs");

  assert.match(source, /@cloudflare\/next-on-pages\/next-dev/);
  assert.match(source, /\bsetupDevPlatform\b/);
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
