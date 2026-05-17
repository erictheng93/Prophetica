#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import { access, constants } from "node:fs/promises";
import { delimiter, resolve } from "node:path";
import process from "node:process";

const repoRoot = resolve(new URL("../..", import.meta.url).pathname);
const evidencePath = resolve(
  repoRoot,
  "docs/evidence/cloudflare-admin-access/latest.json",
);
const terraformPolicyPath = resolve(
  repoRoot,
  "infra/cloudflare/zero-trust/admin-access.tf",
);

const requiredTargetVariables = [
  "CLOUDFLARE_ACCOUNT_ID",
  "CLOUDFLARE_API_TOKEN",
  "TF_VAR_admin_app_domain",
  "TF_VAR_admin_allowed_email_domains",
  "TF_VAR_admin_breakglass_email",
  "TF_VAR_internal_service_token_name",
];

const beforeApply = process.argv.includes("--before-apply");

async function executableExists(name) {
  const paths = (process.env.PATH ?? "").split(delimiter).filter(Boolean);

  for (const path of paths) {
    try {
      await access(resolve(path, name), constants.X_OK);
      return true;
    } catch {
      // Continue scanning PATH.
    }
  }

  return false;
}

function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, "utf8"));
  } catch {
    return null;
  }
}

function hasNonEmptyEnv(name) {
  const value = process.env[name];
  return typeof value === "string" && value.trim().length > 0;
}

function validateEvidenceShape(evidence) {
  const failures = [];

  if (!evidence || typeof evidence !== "object") {
    failures.push("evidence JSON is missing or invalid");
    return failures;
  }

  for (const key of ["plan_log", "apply_log"]) {
    const entry = evidence[key];
    if (!entry || typeof entry !== "object") {
      failures.push(`${key} is missing`);
      continue;
    }
    if (typeof entry.sha256 !== "string" || entry.sha256.length !== 64) {
      failures.push(`${key}.sha256 must be a 64-character hex digest`);
    }
    if (!Number.isInteger(entry.bytes) || entry.bytes <= 0) {
      failures.push(`${key}.bytes must be a positive integer`);
    }
    if (typeof entry.summary !== "string" || entry.summary.trim().length === 0) {
      failures.push(`${key}.summary must be a non-empty success summary`);
    }
  }

  return failures;
}

const terraformAvailable = await executableExists("terraform");
const tofuAvailable = await executableExists("tofu");
const missingEnv = requiredTargetVariables.filter((name) => !hasNonEmptyEnv(name));
const policyExists = existsSync(terraformPolicyPath);
const evidenceExists = existsSync(evidencePath);
const evidence = !beforeApply && evidenceExists ? readJson(evidencePath) : null;
const evidenceFailures = beforeApply
  ? []
  : evidenceExists
    ? validateEvidenceShape(evidence)
    : ["Cloudflare Access plan/apply evidence is missing"];

const failures = [];

if (!terraformAvailable && !tofuAvailable) {
  failures.push("Neither terraform nor tofu is available on PATH");
}

if (!policyExists) {
  failures.push(`Terraform policy file is missing: ${terraformPolicyPath}`);
}

for (const name of missingEnv) {
  failures.push(`Required target-account variable is missing: ${name}`);
}

for (const failure of evidenceFailures) {
  failures.push(failure);
}

const report = {
  status: failures.length === 0 ? "ready" : "blocked",
  mode: beforeApply ? "before_apply" : "full_gate",
  checked_at: new Date().toISOString(),
  terraform_or_tofu_available: terraformAvailable || tofuAvailable,
  terraform_available: terraformAvailable,
  tofu_available: tofuAvailable,
  terraform_policy_path: terraformPolicyPath,
  terraform_policy_exists: policyExists,
  evidence_path: evidencePath,
  evidence_exists: evidenceExists,
  evidence_required: !beforeApply,
  required_target_variables_present: requiredTargetVariables.filter(hasNonEmptyEnv),
  required_target_variables_missing: missingEnv,
  failures,
};

console.log(JSON.stringify(report, null, 2));

if (failures.length > 0) {
  process.exitCode = 1;
}
