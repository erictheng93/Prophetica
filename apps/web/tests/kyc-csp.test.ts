import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

// CSP for KYC routes. The header value is defined in next.config.mjs
// (Next.js loads .mjs configs via vanilla Node, not TS), so this test reads
// the file as text and asserts the directives are present and route-scoped.

const configSource = readFileSync(
  join(process.cwd(), "next.config.mjs"),
  "utf8",
);

test("KYC CSP is scoped to /kyc/:path* (not site-wide) per SPEC-005a section 11.4", () => {
  assert.match(configSource, /source:\s*KYC_ROUTE_GLOB/);
  assert.match(configSource, /KYC_ROUTE_GLOB\s*=\s*"\/kyc\/:path\*"/);
});

test("KYC CSP keeps provider origins out until the real SDK is selected", () => {
  const cspBlock = extractCspBlock(configSource);
  assert.match(configSource, /"script-src 'self'"/);
  assert.match(configSource, /"style-src 'self' 'unsafe-inline'"/);
  assert.match(configSource, /"connect-src 'self'"/);
  assert.match(configSource, /"frame-src 'self'"/);
  assert.match(configSource, /"img-src 'self' data: blob:"/);
  assert.match(configSource, /"media-src 'self' blob:"/);
  assert.match(configSource, /"worker-src 'self' blob:"/);
  assert.ok(
    !/https?:\/\/|wss?:\/\//i.test(cspBlock),
    "CSP must not include KYC provider origins",
  );
});

test("KYC CSP keeps a tight default-src baseline", () => {
  assert.match(configSource, /"default-src 'self'"/);
});

test("KYC CSP must not bless the mock provider with unsafe-eval or wildcard origins", () => {
  const cspBlock = extractCspBlock(configSource);
  assert.ok(
    !cspBlock.includes("'unsafe-eval'"),
    "CSP must not allow unsafe-eval",
  );
  assert.ok(
    !/\s\*(\s|"|;)/.test(cspBlock),
    "CSP must not allow wildcard '*' source",
  );
});

test("KYC CSP is wired through next.config headers", () => {
  assert.match(configSource, /async headers\(\)/);
  assert.match(configSource, /"Content-Security-Policy"/);
  assert.match(configSource, /value:\s*KYC_CSP_HEADER_VALUE/);
});

function extractCspBlock(source: string): string {
  const start = source.indexOf("KYC_CSP_HEADER_VALUE");
  const end = source.indexOf("].join(", start);
  assert.ok(start !== -1 && end !== -1, "CSP block markers not found");
  return source.slice(start, end);
}
