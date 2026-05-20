import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

const entrySource = readFileSync(join(process.cwd(), "app/page.tsx"), "utf8");

test("LIFF entry redirects to the landing journey start for SPEC-023", () => {
  assert.match(entrySource, /import \{ redirect \} from "next\/navigation";/);
  assert.match(entrySource, /const DEFAULT_LANDING_URL =/);
  assert.match(entrySource, /process\.env\.NEXT_PUBLIC_LANDING_URL/);
  assert.match(entrySource, /redirect\(landingUrl\);/);
});

test("LIFF entry does not carry registration login KYC topup or order surfaces", () => {
  assert.doesNotMatch(entrySource, /LiffBootstrap/);
  assert.doesNotMatch(entrySource, /NEXT_PUBLIC_LIFF_ID/);
  assert.doesNotMatch(entrySource, /\/api\/auth\/liff\/session/);
  assert.doesNotMatch(entrySource, /@line\/liff/);
});
