#!/usr/bin/env node
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

import { checkSpec022Complete } from "./check-spec-022-complete.mjs";

const completionEvidenceId = "formal-completion-evidence";

export function finalizeSpec022Complete(repoRoot) {
  const manifestPath = resolve(repoRoot, "docs/spec-022-evidence-manifest.json");
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
          "docs/spec-022-completion-audit.md",
          "docs/spec-022-evidence-manifest.json",
          "docs/evidence/spec-022-liff/latest.json",
          "docs/evidence/spec-022-manual-qa/latest.json",
          "tools/scripts/finalize-spec-022-complete.mjs",
          "tools/scripts/check-spec-022-complete.mjs",
        ],
        evidence:
          "External LIFF and manual QA evidence were recorded, then rtk node tools/scripts/finalize-spec-022-complete.mjs and rtk node tools/scripts/check-spec-022-complete.mjs passed.",
      },
    ],
  };

  writeFileSync(manifestPath, `${JSON.stringify(candidate, null, 2)}\n`);
  const result = checkSpec022Complete(repoRoot);
  if (!result.complete) {
    writeFileSync(manifestPath, original);
    return result;
  }

  return result;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const repoRoot = resolve(process.argv[2] ?? new URL("../..", import.meta.url).pathname);
  const result = finalizeSpec022Complete(repoRoot);
  if (!result.complete) {
    for (const error of result.errors) {
      console.error(error);
    }
    process.exit(1);
  }

  console.log(
    `SPEC-022 manifest finalized (${result.completedEvidenceCount} evidence entries).`,
  );
}
