import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const repoRoot = path.resolve(import.meta.dirname, "../..");
const recorderScript = path.join(
  repoRoot,
  "tools/scripts/record-cloudflare-admin-access-evidence.mjs",
);
const evidenceDir = path.join(repoRoot, "docs/evidence/cloudflare-admin-access");

test("records Cloudflare target metadata without leaking API token", async () => {
  await cleanEvidence();
  const root = await mkdtemp(path.join(os.tmpdir(), "prophetica-cf-access-"));
  const planLog = path.join(root, "plan.log");
  const applyLog = path.join(root, "apply.log");
  await writeFile(planLog, "Plan: 5 to add, 0 to change, 0 to destroy.\n");
  await writeFile(
    applyLog,
    "Apply complete! Resources: 5 added, 0 changed, 0 destroyed.\n",
  );

  execFileSync(process.execPath, [recorderScript, planLog, applyLog], {
    cwd: repoRoot,
    env: {
      ...process.env,
      CLOUDFLARE_ACCOUNT_ID: "0123456789abcdef0123456789abcdef",
      CLOUDFLARE_API_TOKEN: "cf-secret-token-should-not-appear",
      TF_VAR_admin_app_domain: "admin.example.com",
      TF_VAR_admin_allowed_email_domains: "example.com,example.org",
      TF_VAR_admin_breakglass_email: "breakglass@example.com",
      TF_VAR_internal_service_token_name: "prophetica-admin-access",
    },
  });

  const rawEvidence = await readFile(path.join(evidenceDir, "latest.json"), "utf8");
  const evidence = JSON.parse(rawEvidence);

  assert.equal(evidence.target_account.admin_app_domain, "admin.example.com");
  assert.equal(
    evidence.target_account.internal_service_token_name,
    "prophetica-admin-access",
  );
  assert.equal(evidence.target_account.account_id_suffix, "abcdef");
  assert.match(evidence.target_account.account_id_sha256, /^[a-f0-9]{64}$/);
  assert.match(
    evidence.target_account.allowed_email_domains_sha256,
    /^[a-f0-9]{64}$/,
  );
  assert.match(evidence.target_account.breakglass_email_sha256, /^[a-f0-9]{64}$/);
  assert.doesNotMatch(rawEvidence, /cf-secret-token-should-not-appear/);
  assert.doesNotMatch(rawEvidence, /breakglass@example\.com/);

  await cleanEvidence();
});

test("requires Cloudflare API token before recording target evidence", async () => {
  await cleanEvidence();
  const root = await mkdtemp(path.join(os.tmpdir(), "prophetica-cf-access-"));
  const planLog = path.join(root, "plan.log");
  const applyLog = path.join(root, "apply.log");
  await writeFile(planLog, "Plan: 5 to add, 0 to change, 0 to destroy.\n");
  await writeFile(
    applyLog,
    "Apply complete! Resources: 5 added, 0 changed, 0 destroyed.\n",
  );

  const result = spawnSync(process.execPath, [recorderScript, planLog, applyLog], {
    cwd: repoRoot,
    env: {
      ...process.env,
      CLOUDFLARE_ACCOUNT_ID: "0123456789abcdef0123456789abcdef",
      CLOUDFLARE_API_TOKEN: "",
      TF_VAR_admin_app_domain: "admin.example.com",
      TF_VAR_admin_allowed_email_domains: "example.com,example.org",
      TF_VAR_admin_breakglass_email: "breakglass@example.com",
      TF_VAR_internal_service_token_name: "prophetica-admin-access",
    },
    encoding: "utf8",
  });

  assert.equal(result.status, 1);
  assert.match(
    result.stderr,
    /Required target-account environment variable is missing: CLOUDFLARE_API_TOKEN/,
  );

  await assert.rejects(
    readFile(path.join(evidenceDir, "latest.json"), "utf8"),
    /ENOENT/,
  );
});

async function cleanEvidence() {
  await mkdir(path.join(repoRoot, "docs/evidence"), { recursive: true });
  await mkdir(evidenceDir, { recursive: true });
  await rm(path.join(evidenceDir, "latest.json"), { force: true });
}
