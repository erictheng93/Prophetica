#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "../..");
const manifestPath = resolve(repoRoot, "docs/spec-022-evidence-manifest.json");
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
const blockers = Array.isArray(manifest.blocking_items)
  ? manifest.blocking_items
  : [];

if (manifest.status !== "complete" || blockers.length > 0) {
  console.error("SPEC-022 Login Event Intelligence is not complete.");
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
  console.error("SPEC-022 manifest has no completed evidence.");
  process.exit(1);
}

const requiredExternalEvidence = Array.isArray(manifest.required_external_evidence)
  ? manifest.required_external_evidence
  : [];
for (const requirement of requiredExternalEvidence) {
  validateEvidenceFile(requirement);
}

console.log(
  `SPEC-022 manifest is complete (${completedEvidence.length} evidence entries).`,
);

function validateEvidenceFile(requirement) {
  const relativePath = requirement?.path;
  const requiredFields = Array.isArray(requirement?.required_fields)
    ? requirement.required_fields
    : [];
  if (typeof relativePath !== "string" || relativePath.trim().length === 0) {
    console.error("SPEC-022 manifest has an invalid external evidence path.");
    process.exit(1);
  }
  if (requiredFields.length === 0) {
    console.error(`SPEC-022 evidence requirement has no fields: ${relativePath}`);
    process.exit(1);
  }

  const evidencePath = resolve(repoRoot, relativePath);
  if (!existsSync(evidencePath)) {
    console.error(`Missing SPEC-022 evidence: ${relativePath}`);
    process.exit(1);
  }

  const evidence = JSON.parse(readFileSync(evidencePath, "utf8"));
  for (const field of requiredFields) {
    if (evidence[field] !== true) {
      console.error(
        `SPEC-022 evidence ${relativePath} does not satisfy required field: ${field}`,
      );
      process.exit(1);
    }
  }
}
