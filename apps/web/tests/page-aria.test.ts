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

test("static forms do not use unsupported aria-disabled attributes", () => {
  for (const file of formPageFiles) {
    const source = readFileSync(join(process.cwd(), file), "utf8");

    assert.doesNotMatch(source, /<form\b[^>]*\baria-disabled=/, file);
  }
});
