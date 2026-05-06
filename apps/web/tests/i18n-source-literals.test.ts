import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const migratedFiles = [
  "app/auth/sign-in/page.tsx",
  "app/legal/aml/page.tsx",
  "app/legal/geo-block/page.tsx",
  "app/legal/privacy/page.tsx",
  "app/legal/terms/page.tsx",
  "components/stub-page.tsx",
];

test("migrated auth, legal, and shared placeholder copy is sourced from messages", () => {
  for (const file of migratedFiles) {
    const source = readFileSync(join(process.cwd(), file), "utf8");

    assert.doesNotMatch(source, /[\u4e00-\u9fff]/, file);
  }
});
