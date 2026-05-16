import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";

const messages = JSON.parse(
  readFileSync(join(process.cwd(), "messages/zh-Hant.json"), "utf8"),
) as Record<string, unknown>;

// Files that have completed next-intl migration. The assertion below scans
// each for CJK characters in *runtime* code — comments are stripped first so
// that source comments containing intentional literals are not treated as UI copy.
// stay legal documentation without breaking the gate.
const migratedFiles = [
  "app/auth/sign-in/page.tsx",
  "app/kyc/layout.tsx",
  "app/kyc/page.tsx",
  "app/kyc/privacy-notice/page.tsx",
  "app/kyc/tier-2/page.tsx",
  "app/kyc/tier-2/tier-2-launch-client.tsx",
  "app/kyc/tier-3/page.tsx",
  "app/kyc/tier-3/tier-3-launch-client.tsx",
  "app/legal/privacy/page.tsx",
  "app/legal/terms/page.tsx",
  "app/markets/[id]/not-found.tsx",
  "app/markets/[id]/page.tsx",
  "app/markets/layout.tsx",
  "app/markets/new/page.tsx",
  "app/markets/page.tsx",
  "app/markets/settled/page.tsx",
  "app/markets/trending/page.tsx",
  "app/page.tsx",
  "app/responsible-gaming/layout.tsx",
  "app/responsible-gaming/limits/page.tsx",
  "app/responsible-gaming/page.tsx",
  "app/support/disputes/page.tsx",
  "app/topup/page.tsx",
  "components/market-card.tsx",
  "components/probability-chart.tsx",
  "components/stub-page.tsx",
  "components/tier-stepper.tsx",
  "components/trade-panel.tsx",
];

// Strip /* ... */ block comments and // line comments. Heuristic — does not
// understand strings containing `//`, but we don't write URL literals with
// hard-coded protocols in this codebase. Good enough to keep CJK comments
// (SPEC references, design notes) out of the runtime-string check.
function stripComments(source: string): string {
  return source
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/(^|[^:"'`\\])\/\/.*$/gm, "$1");
}

test("migrated copy is sourced from messages, not inline literals", () => {
  for (const file of migratedFiles) {
    const source = readFileSync(join(process.cwd(), file), "utf8");
    const runtime = stripComments(source);

    assert.doesNotMatch(runtime, /[一-鿿]/, file);
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
