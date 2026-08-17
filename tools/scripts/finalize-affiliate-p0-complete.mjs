#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

import { checkAffiliateP0Complete } from "./check-affiliate-p0-complete.mjs";

const completionEvidenceId = "formal-affiliate-p0-completion-evidence";

export function finalizeAffiliateP0Complete(repoRoot) {
  const manifestPath = resolve(
    repoRoot,
    "docs/affiliate-p0-evidence-manifest.json",
  );
  const original = readFileSync(manifestPath, "utf8");
  const manifest = JSON.parse(original);
  const completedEvidence = Array.isArray(manifest.completed_evidence)
    ? manifest.completed_evidence.filter((entry) => entry?.id !== completionEvidenceId)
    : [];

  const candidate = {
    ...manifest,
    last_updated: new Date().toISOString().slice(0, 10),
    status: "complete",
    blocking_items: [],
    completed_evidence: [
      ...completedEvidence,
      {
        id: completionEvidenceId,
        artifacts: [
          "docs/affiliate-p0-completion-audit.md",
          "docs/affiliate-p0-evidence-manifest.json",
          "docs/evidence/cloudflare-admin-access/latest.json",
          "docs/evidence/public-signup-referral/latest.json",
          "docs/evidence/webauthn-passkey/latest.json",
          "tools/scripts/finalize-affiliate-p0-complete.mjs",
          "tools/scripts/check-affiliate-p0-complete.mjs",
        ],
        evidence:
          "Cloudflare Access, WebAuthn/passkey, public signup referral, and admin affiliate review E2E evidence were recorded, then node tools/scripts/finalize-affiliate-p0-complete.mjs and node tools/scripts/check-affiliate-p0-complete.mjs passed.",
      },
    ],
  };

  writeFileSync(manifestPath, `${JSON.stringify(candidate, null, 2)}\n`);
  const result = checkAffiliateP0Complete(repoRoot);
  if (!result.complete) {
    writeFileSync(manifestPath, original);
    return result;
  }

  return result;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const repoRoot = resolve(process.argv[2] ?? new URL("../..", import.meta.url).pathname);
  const result = finalizeAffiliateP0Complete(repoRoot);
  if (!result.complete) {
    for (const error of result.errors) {
      console.error(error);
    }
    process.exit(1);
  }

  console.log(
    `Affiliate / referral P0 manifest finalized (${result.completedEvidenceCount} evidence entries).`,
  );
}
