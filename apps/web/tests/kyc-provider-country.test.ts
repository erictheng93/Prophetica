import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const tier2Page = readFileSync(
  join(process.cwd(), "app/kyc/tier-2/page.tsx"),
  "utf8",
);
const tier3Page = readFileSync(
  join(process.cwd(), "app/kyc/tier-3/page.tsx"),
  "utf8",
);

test("Tier 2 iProov launch uses the verified Tier 1 country from KYC overview", () => {
  assert.match(tier2Page, /const country = overview\.country \?\? "TW";/);
  assert.doesNotMatch(tier2Page, /const country = "TW";/);
});

test("Tier 3 iProov launch uses the verified Tier 1 country from KYC overview", () => {
  assert.match(tier3Page, /const country = overview\.country \?\? "TW";/);
  assert.doesNotMatch(tier3Page, /const country = "TW";/);
});
