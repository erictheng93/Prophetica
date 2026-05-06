import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const formPageFiles = [
  "app/auth/sign-in/page.tsx",
  "app/support/contact/page.tsx",
  "app/support/disputes/page.tsx",
  "app/topup/page.tsx",
];
const kycFormPageFiles = [
  "app/kyc/tier-1/page.tsx",
  "app/kyc/tier-2/page.tsx",
  "app/kyc/tier-3/page.tsx",
];

test("static forms do not use unsupported aria-disabled attributes", () => {
  for (const file of formPageFiles) {
    const source = readFileSync(join(process.cwd(), file), "utf8");

    assert.doesNotMatch(source, /<form\b[^>]*\baria-disabled=/, file);
  }
});

test("KYC action forms are not shipped disabled", () => {
  for (const file of kycFormPageFiles) {
    const source = readFileSync(join(process.cwd(), file), "utf8");

    assert.doesNotMatch(source, /<fieldset\b[^>]*\bdisabled\b/, file);
    assert.doesNotMatch(source, /<button\b[^>]*\bdisabled\b/, file);
  }
});
