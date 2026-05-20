import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import test from "node:test";

const source = readFileSync(
  join(process.cwd(), "app/auth/sign-in/page.tsx"),
  "utf8",
);

test("sign-in page sanitizes return_to before rendering or redirecting", () => {
  assert.match(
    source,
    /import \{ safeReturnTo \} from "@\/lib\/safe-return-to"/,
  );
  assert.match(source, /const returnTo = safeReturnTo\(rawReturnTo\);/);
  assert.match(
    source,
    /const returnTo = safeReturnTo\(readFormValue\(formData, "return_to"\)\);/,
  );
});

test("sign-in and signup forms persist sanitized return_to as a hidden field", () => {
  assert.equal(
    source.match(
      /<input type="hidden" name="return_to" value=\{returnTo\} \/>/g,
    )?.length,
    2,
  );
});

test("successful sign-in redirects to the sanitized return_to target", () => {
  assert.match(source, /redirect\(returnTo\);/);
  assert.match(
    source,
    /const nextPath = authPath\(\{ signup: "registered", returnTo \}\);/,
  );
  assert.match(source, /redirect\(nextPath\);/);
  assert.match(source, /recordJourneyRedirect\("\/auth\/sign-in", returnTo\);/);
});

test("KYC Tier 1 completion preserves sanitized return_to for intercepted journeys", () => {
  const tier1Source = readFileSync(
    join(process.cwd(), "app/kyc/tier-1/page.tsx"),
    "utf8",
  );

  assert.match(
    tier1Source,
    /import \{ safeReturnTo \} from "@\/lib\/safe-return-to";/,
  );
  assert.match(
    tier1Source,
    /const returnTo = safeReturnTo\(\s*readFormValue\(formData, "return_to"\),\s*"\/kyc\/status",\s*\);/,
  );
  assert.match(
    tier1Source,
    /<input type="hidden" name="return_to" value=\{returnTo\} \/>/,
  );
  assert.match(tier1Source, /redirect\(returnTo\);/);
});
