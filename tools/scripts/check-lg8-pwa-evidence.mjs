#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "../..");
const evidencePath = resolve(repoRoot, "docs/evidence/lg-8-pwa/latest.json");

if (!existsSync(evidencePath)) {
  console.error("Missing LG-8 PWA evidence: docs/evidence/lg-8-pwa/latest.json");
  process.exit(1);
}

const evidence = JSON.parse(readFileSync(evidencePath, "utf8"));
const failures = [];

if (!/^https:\/\//i.test(String(evidence.final_url || evidence.requested_url || ""))) {
  failures.push("Evidence URL must be HTTPS.");
}
if (evidence.pwa_score !== 1) {
  failures.push("Evidence pwa_score must equal 1.");
}
if (!/^[a-f0-9]{64}$/.test(String(evidence.source_report?.sha256 ?? ""))) {
  failures.push("Evidence source_report.sha256 must be present.");
}

if (failures.length > 0) {
  console.error("LG-8 PWA evidence check failed.");
  for (const failure of failures) {
    console.error(failure);
  }
  process.exit(1);
}

console.log("LG-8 PWA evidence check passed.");
