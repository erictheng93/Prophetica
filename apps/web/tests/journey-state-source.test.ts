import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

const source = readFileSync(
  join(process.cwd(), "lib/journey-state.ts"),
  "utf8",
);

test("journey-state exposes the SPEC-023 typed visitor/authenticated shape", () => {
  assert.match(
    source,
    /export type JourneyStatus = "visitor" \| "authenticated";/,
  );
  assert.match(source, /kycTier\?: number;/);
  assert.match(source, /balanceMinor\?: number;/);
});

test("getJourneyState derives authenticated state from server viewer session", () => {
  assert.match(
    source,
    /import \{ getUpstreamSessionToken, getViewer \} from "\.\/session";/,
  );
  assert.match(source, /import \{ fetchKycOverview \} from "\.\/kyc-api";/);
  assert.match(
    source,
    /import \{ fetchWalletOverview \} from "\.\/api-gateway";/,
  );
  assert.match(source, /export async function getJourneyState/);
  assert.match(source, /status: "visitor"/);
  assert.match(source, /status: "authenticated"/);
  assert.match(source, /userId: viewer\.userId/);
  assert.match(source, /kycTier: kycOverview\.currentTierNumber/);
  assert.match(
    source,
    /balanceMinor: pointsToMinorUnits\(walletOverview\.balance\.availablePoints\)/,
  );
  assert.match(source, /allowUserIdFallback: devBffFallbackEnabled\(\)/);
  assert.doesNotMatch(
    source,
    /process\.env\.NODE_ENV !== "production"/,
    "SPEC-023 H-1: dev BFF fallback must use positive opt-in flag, not negative NODE_ENV gate",
  );
});
