#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const manifestPath = resolve(
  import.meta.dirname,
  "../../docs/affiliate-p0-evidence-manifest.json",
);
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
const blockers = Array.isArray(manifest.blocking_items)
  ? manifest.blocking_items
  : [];

if (manifest.status !== "complete" || blockers.length > 0) {
  console.error("Affiliate / referral P0 is not complete.");
  console.error(`Manifest status: ${manifest.status ?? "unknown"}`);
  if (blockers.length > 0) {
    console.error("Blocking items:");
    for (const blocker of blockers) {
      console.error(`- ${blocker.id}: ${blocker.reason}`);
    }
  }
  process.exit(1);
}

const completedEvidence = Array.isArray(manifest.completed_evidence)
  ? manifest.completed_evidence
  : [];

if (completedEvidence.length === 0) {
  console.error("Affiliate / referral P0 manifest has no completed evidence.");
  process.exit(1);
}

console.log(
  `Affiliate / referral P0 manifest is complete (${completedEvidence.length} evidence entries).`,
);

