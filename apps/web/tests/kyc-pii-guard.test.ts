import assert from "node:assert/strict";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

// SPEC-005a §7.1 + SPEC-007 §4.3 — KYC client code must not write
// applicant_id, sdk_token, raw SumSub event payloads, or document data to
// the browser console. The structured logger in lib/observability/logger.ts
// is the only sanctioned channel.
//
// This test scans every .ts/.tsx file under app/kyc/** (the surfaces where
// the SumSub SDK and KYC state machine actually run) and forbids
// `console.log` (allowing `console.warn` / `console.error` for genuine
// failure paths). Comments are stripped first so SPEC references and
// design notes that quote `console.log(...)` literally are ignored.

const KYC_APP_ROOT = join(process.cwd(), "app/kyc");

function* walkSourceFiles(dir: string): Generator<string> {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const stat = statSync(full);
    if (stat.isDirectory()) {
      yield* walkSourceFiles(full);
    } else if (full.endsWith(".ts") || full.endsWith(".tsx")) {
      yield full;
    }
  }
}

function stripComments(source: string): string {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/(^|[^:"'`\\])\/\/.*$/gm, "$1");
}

test("no console.log inside app/kyc/** — only warn / error are sanctioned", () => {
  let scanned = 0;
  for (const file of walkSourceFiles(KYC_APP_ROOT)) {
    const runtime = stripComments(readFileSync(file, "utf8"));
    assert.doesNotMatch(
      runtime,
      /\bconsole\.log\s*\(/,
      `${file}: console.log is forbidden — route through lib/observability/logger.ts (SPEC-007 §4.1) instead`,
    );
    scanned++;
  }
  assert.ok(scanned > 0, "expected to scan at least one file under app/kyc");
});

test("no PII-shaped string literals inside app/kyc/** runtime code", () => {
  // Belt-and-suspenders complement to logger's HARD_DENY_KEYS — catches the
  // failure mode where someone hand-writes `\`applicant_id=${id}\`` in JSX
  // or telemetry without going through the logger or metrics validator.
  const FORBIDDEN_TOKENS = [
    "applicant_id",
    "sdk_token",
    "sdkToken =", // assigned, not just a typed param
  ] as const;

  for (const file of walkSourceFiles(KYC_APP_ROOT)) {
    const runtime = stripComments(readFileSync(file, "utf8"));
    for (const token of FORBIDDEN_TOKENS) {
      const matches = [...runtime.matchAll(new RegExp(`['"\`]${token}`, "g"))];
      assert.equal(
        matches.length,
        0,
        `${file}: literal '${token}' must not appear in runtime strings — see SPEC-005a §7.1`,
      );
    }
  }
});
