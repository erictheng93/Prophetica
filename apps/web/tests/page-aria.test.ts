import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const formPageFiles = [
  "app/auth/sign-in/page.tsx",
  "app/responsible-gaming/self-exclusion/page.tsx",
  "app/support/contact/page.tsx",
  "app/support/disputes/page.tsx",
  "app/topup/page.tsx",
];
const kycFormPageFiles = [
  "app/kyc/tier-1/page.tsx",
  "app/kyc/tier-2/page.tsx",
  "app/kyc/tier-3/page.tsx",
];
const actionFormPageFiles = [
  ...kycFormPageFiles,
  "app/responsible-gaming/self-exclusion/page.tsx",
];

test("static forms do not use unsupported aria-disabled attributes", () => {
  for (const file of formPageFiles) {
    const source = readFileSync(join(process.cwd(), file), "utf8");

    assert.doesNotMatch(source, /<form\b[^>]*\baria-disabled=/, file);
  }
});

test("action forms are not shipped disabled", () => {
  for (const file of actionFormPageFiles) {
    const source = readFileSync(join(process.cwd(), file), "utf8");

    assert.doesNotMatch(source, /<fieldset\b[^>]*\bdisabled\b/, file);
    assert.doesNotMatch(source, /<button\b[^>]*\bdisabled\b/, file);
  }
});

test("trade panel submit is not validity-disabled before hydration validation runs", () => {
  const source = readFileSync(
    join(process.cwd(), "components/trade-panel.tsx"),
    "utf8",
  );

  assert.doesNotMatch(source, /disabled=\{!isValid \|\| isSubmitting\}/);
});

test("trade panel uses an explicit click handler instead of native form submission", () => {
  const source = readFileSync(
    join(process.cwd(), "components/trade-panel.tsx"),
    "utf8",
  );

  assert.doesNotMatch(source, /type="submit"/);
  assert.match(source, /onClick=\{handleSubmit\(onSubmit\)\}/);
});

test("trade panel has a concrete default side for hydrated submission", () => {
  const source = readFileSync(
    join(process.cwd(), "components/trade-panel.tsx"),
    "utf8",
  );

  assert.match(source, /value="YES"[\s\S]*defaultChecked/);
});
