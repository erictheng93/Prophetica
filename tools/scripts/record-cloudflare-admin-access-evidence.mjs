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

const evidence = {
  recorded_at: new Date().toISOString(),
  scope: "Cloudflare Zero Trust admin Access policy",
  expected_policy: "infra/cloudflare/zero-trust/admin-access.tf",
  structural_verifier: "tools/scripts/validate-cloudflare-admin-access.mjs",
  plan_log: {
    file: basename(planLogPath),
    sha256: sha256(planLog),
    bytes: planLog.byteLength,
  },
  apply_log: {
    file: basename(applyLogPath),
    sha256: sha256(applyLog),
    bytes: applyLog.byteLength,
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

