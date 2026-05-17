#!/usr/bin/env node
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, resolve } from "node:path";

const [, , planLogArg, applyLogArg] = process.argv;

if (!planLogArg || !applyLogArg) {
  console.error(
    "Usage: node tools/scripts/record-cloudflare-admin-access-evidence.mjs <terraform-plan-log> <terraform-apply-log>",
  );
  process.exit(2);
}

const repoRoot = resolve(import.meta.dirname, "../..");
const evidenceDir = resolve(repoRoot, "docs/evidence/cloudflare-admin-access");
const planLogPath = resolve(process.cwd(), planLogArg);
const applyLogPath = resolve(process.cwd(), applyLogArg);

const planLog = readFileSync(planLogPath);
const applyLog = readFileSync(applyLogPath);
const planSummary = validatePlanLog(planLog);
const applySummary = validateApplyLog(applyLog);

const evidence = {
  recorded_at: new Date().toISOString(),
  scope: "Cloudflare Zero Trust admin Access policy",
  expected_policy: "infra/cloudflare/zero-trust/admin-access.tf",
  structural_verifier: "tools/scripts/validate-cloudflare-admin-access.mjs",
  plan_log: {
    file: basename(planLogPath),
    sha256: sha256(planLog),
    bytes: planLog.byteLength,
    summary: planSummary,
  },
  apply_log: {
    file: basename(applyLogPath),
    sha256: sha256(applyLog),
    bytes: applyLog.byteLength,
    summary: applySummary,
  },
};

mkdirSync(evidenceDir, { recursive: true });
writeFileSync(
  resolve(evidenceDir, "latest.json"),
  `${JSON.stringify(evidence, null, 2)}\n`,
);

console.log(`Cloudflare admin Access evidence recorded: ${resolve(evidenceDir, "latest.json")}`);

function sha256(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}

function validatePlanLog(buffer) {
  const text = buffer.toString("utf8");

  if (text.trim().length === 0) {
    fail("Terraform plan log is empty");
  }
  if (/\bError:/i.test(text)) {
    fail("Terraform plan log contains an error marker");
  }

  const changeSummary = text.match(/Plan:\s+\d+\s+to add,\s+\d+\s+to change,\s+\d+\s+to destroy\./);
  if (changeSummary) {
    return changeSummary[0];
  }

  if (/No changes\.\s+Your infrastructure matches the configuration\./.test(text)) {
    return "No changes. Your infrastructure matches the configuration.";
  }

  fail("Terraform plan log does not contain a recognized success summary");
}

function validateApplyLog(buffer) {
  const text = buffer.toString("utf8");

  if (text.trim().length === 0) {
    fail("Terraform apply log is empty");
  }
  if (/\bError:/i.test(text)) {
    fail("Terraform apply log contains an error marker");
  }

  const applySummary = text.match(
    /Apply complete!\s+Resources:\s+\d+\s+added,\s+\d+\s+changed,\s+\d+\s+destroyed\./,
  );
  if (applySummary) {
    return applySummary[0];
  }

  fail("Terraform apply log does not contain an apply success summary");
}

function fail(message) {
  console.error(message);
  process.exit(1);
}
