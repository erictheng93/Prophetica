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
  target_account: targetAccountEvidence(process.env),
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

function sha256Text(value) {
  return sha256(Buffer.from(value, "utf8"));
}

function targetAccountEvidence(env) {
  const accountId = requiredEnv(env, "CLOUDFLARE_ACCOUNT_ID");
  requiredEnv(env, "CLOUDFLARE_API_TOKEN");
  const adminAppDomain = requiredEnv(env, "TF_VAR_admin_app_domain");
  const allowedEmailDomains = requiredEnv(
    env,
    "TF_VAR_admin_allowed_email_domains",
  );
  const breakglassEmail = requiredEnv(env, "TF_VAR_admin_breakglass_email");
  const internalServiceTokenName = requiredEnv(
    env,
    "TF_VAR_internal_service_token_name",
  );

  return {
    account_id_sha256: sha256Text(accountId),
    account_id_suffix: accountId.slice(-6),
    admin_app_domain: adminAppDomain,
    allowed_email_domains_sha256: sha256Text(allowedEmailDomains),
    breakglass_email_sha256: sha256Text(breakglassEmail),
    internal_service_token_name: internalServiceTokenName,
  };
}

function requiredEnv(env, name) {
  const value = env[name];
  if (typeof value !== "string" || value.trim().length === 0) {
    fail(`Required target-account environment variable is missing: ${name}`);
  }
  return value.trim();
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
