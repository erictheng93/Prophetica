#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

export function checkSpec022Complete(repoRoot) {
  const manifestPath = resolve(repoRoot, "docs/spec-022-evidence-manifest.json");
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const errors = [];
  const blockers = Array.isArray(manifest.blocking_items)
    ? manifest.blocking_items
    : [];

  if (manifest.status !== "complete" || blockers.length > 0) {
    errors.push("SPEC-022 Login Event Intelligence is not complete.");
    errors.push(`Manifest status: ${manifest.status ?? "unknown"}`);
    if (blockers.length > 0) {
      errors.push("Blocking items:");
      for (const blocker of blockers) {
        errors.push(`- ${blocker.id}: ${blocker.reason}`);
      }
    }
    return { complete: false, completedEvidenceCount: 0, errors };
  }

  const completionAudit = manifest.completion_audit;
  if (
    typeof completionAudit !== "string" ||
    completionAudit.trim().length === 0 ||
    !existsSync(resolve(repoRoot, completionAudit))
  ) {
    errors.push("SPEC-022 manifest is missing a valid completion_audit file.");
  }

  const completedEvidence = Array.isArray(manifest.completed_evidence)
    ? manifest.completed_evidence
    : [];
  if (completedEvidence.length === 0) {
    errors.push("SPEC-022 manifest has no completed evidence.");
  }
  for (const entry of completedEvidence) {
    validateCompletedEvidence(repoRoot, entry, errors);
  }

  const requiredExternalEvidence = Array.isArray(manifest.required_external_evidence)
    ? manifest.required_external_evidence
    : [];
  for (const requirement of requiredExternalEvidence) {
    validateEvidenceFile(repoRoot, requirement, errors);
  }

  return {
    complete: errors.length === 0,
    completedEvidenceCount: completedEvidence.length,
    errors,
  };
}

function validateCompletedEvidence(repoRoot, entry, errors) {
  if (typeof entry?.id !== "string" || entry.id.trim().length === 0) {
    errors.push("SPEC-022 completed evidence entry is missing an id.");
  }
  if (typeof entry?.evidence !== "string" || entry.evidence.trim().length === 0) {
    errors.push(`SPEC-022 completed evidence ${entry?.id ?? "(unknown)"} has no evidence note.`);
  }
  const artifacts = Array.isArray(entry?.artifacts) ? entry.artifacts : [];
  if (artifacts.length === 0) {
    errors.push(`SPEC-022 completed evidence ${entry?.id ?? "(unknown)"} has no artifacts.`);
  }
  for (const artifact of artifacts) {
    if (
      typeof artifact !== "string" ||
      artifact.trim().length === 0 ||
      !existsSync(resolve(repoRoot, artifact))
    ) {
      errors.push(
        `SPEC-022 completed evidence ${entry?.id ?? "(unknown)"} references missing artifact: ${artifact}`,
      );
    }
  }
}

function validateEvidenceFile(repoRoot, requirement, errors) {
  const relativePath = requirement?.path;
  const requiredFields = Array.isArray(requirement?.required_fields)
    ? requirement.required_fields
    : [];
  if (typeof relativePath !== "string" || relativePath.trim().length === 0) {
    errors.push("SPEC-022 manifest has an invalid external evidence path.");
    return;
  }
  if (requiredFields.length === 0) {
    errors.push(`SPEC-022 evidence requirement has no fields: ${relativePath}`);
    return;
  }

  const evidencePath = resolve(repoRoot, relativePath);
  if (!existsSync(evidencePath)) {
    errors.push(`Missing SPEC-022 evidence: ${relativePath}`);
    return;
  }

  const evidence = JSON.parse(readFileSync(evidencePath, "utf8"));
  for (const field of requiredFields) {
    if (evidence[field] !== true) {
      errors.push(
        `SPEC-022 evidence ${relativePath} does not satisfy required field: ${field}`,
      );
    }
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const repoRoot = resolve(import.meta.dirname, "../..");
  const result = checkSpec022Complete(repoRoot);
  if (!result.complete) {
    for (const error of result.errors) {
      console.error(error);
    }
    process.exit(1);
  }
  console.log(
    `SPEC-022 manifest is complete (${result.completedEvidenceCount} evidence entries).`,
  );
}
