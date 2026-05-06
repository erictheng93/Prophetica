import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const messages = JSON.parse(
  readFileSync(join(process.cwd(), "messages/zh-Hant.json"), "utf8"),
) as Record<string, unknown>;

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

test("KYC page message namespaces are attached to kyc, not responsibleGaming", () => {
  const kyc = messages.kyc as Record<string, unknown>;
  const responsibleGaming = messages.responsibleGaming as Record<string, unknown>;
  const expectedKycNamespaces = [
    "layout",
    "tierStepper",
    "overview",
    "tier1",
    "tier2",
    "tier3",
    "privacyNotice",
    "status",
  ];

  for (const namespace of expectedKycNamespaces) {
    assert.ok(kyc[namespace], `missing kyc.${namespace}`);
  }

  for (const misplacedNamespace of [
    "tier1",
    "tier2",
    "tier3",
    "privacyNotice",
    "status",
  ]) {
    assert.equal(
      responsibleGaming[misplacedNamespace],
      undefined,
      `responsibleGaming.${misplacedNamespace} should not contain KYC copy`,
    );
  }
});
