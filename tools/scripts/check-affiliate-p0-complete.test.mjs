import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { checkAffiliateP0Complete } from "./check-affiliate-p0-complete.mjs";

const repoRoot = path.resolve(import.meta.dirname, "../..");
const finalizeScript = path.join(
  repoRoot,
  "tools/scripts/finalize-affiliate-p0-complete.mjs",
);

async function makeRoot() {
  const root = await mkdtemp(path.join(os.tmpdir(), "prophetica-affiliate-p0-"));
  await mkdir(path.join(root, "docs/evidence/cloudflare-admin-access"), {
    recursive: true,
  });
  await mkdir(path.join(root, "docs/evidence/webauthn-passkey"), {
    recursive: true,
  });
  await mkdir(path.join(root, "docs/evidence/public-signup-referral"), {
    recursive: true,
  });
  await mkdir(path.join(root, "apps/admin/tests/e2e"), { recursive: true });
  await mkdir(path.join(root, "apps/admin/app/affiliate/commissions"), {
    recursive: true,
  });
  await mkdir(path.join(root, "apps/admin/app/affiliate/payouts"), {
    recursive: true,
  });
  await mkdir(path.join(root, "apps/admin/app/affiliate"), { recursive: true });
  await mkdir(path.join(root, "apps/admin/lib"), { recursive: true });
  await mkdir(path.join(root, "tools/scripts"), { recursive: true });
  await writeFile(path.join(root, "docs/affiliate-p0-completion-audit.md"), "# audit\n");
  await writeFile(
    path.join(root, "tools/scripts/check-affiliate-p0-complete.mjs"),
    "// check\n",
  );
  await writeFile(
    path.join(root, "tools/scripts/finalize-affiliate-p0-complete.mjs"),
    "// finalize\n",
  );

  await writeFile(
    path.join(root, "docs/evidence/cloudflare-admin-access/latest.json"),
    `${JSON.stringify({
      target_account: validTargetAccountEvidence(),
      plan_log: validLogEvidence(),
      apply_log: validLogEvidence(),
    })}\n`,
  );
  await writeFile(
    path.join(root, "docs/evidence/webauthn-passkey/latest.json"),
    `${JSON.stringify({
      registration_round_trip: true,
      registration_replay_rejected: true,
      assertion_round_trip: true,
      assertion_replay_rejected: true,
      wrong_origin_rejected: true,
      unknown_credential_rejected: true,
      revoked_credential_rejected: true,
      fresh_hardware_mfa_session: true,
      admin_step_up_fresh_accepts: true,
      admin_step_up_stale_rejects: true,
    })}\n`,
  );
  await writeFile(
    path.join(root, "docs/evidence/public-signup-referral/latest.json"),
    `${JSON.stringify({
      signup_submit_path: true,
      referral_cookie_submitted: true,
      registration_endpoint_confirmed: true,
      durable_login_confirmed: true,
      referral_capture_confirmed: true,
    })}\n`,
  );
  await writeFile(
    path.join(root, "apps/admin/tests/e2e/affiliate-review.spec.ts"),
    "// affiliate review e2e\n",
  );
  await writeFile(
    path.join(root, "apps/admin/app/affiliate/commissions/page.tsx"),
    "// commissions page\n",
  );
  await writeFile(
    path.join(root, "apps/admin/app/affiliate/payouts/page.tsx"),
    "// payouts page\n",
  );
  await writeFile(
    path.join(root, "apps/admin/app/affiliate/actions.ts"),
    "// affiliate actions\n",
  );
  await writeFile(
    path.join(root, "apps/admin/lib/affiliate-management-bff.ts"),
    "// affiliate bff\n",
  );
  return root;
}

test("requires admin affiliate review E2E evidence before completing P0", async () => {
  const root = await makeRoot();
  await writeManifest(root, {
    status: "complete",
    blocking_items: [],
    completed_evidence: [
      {
        id: "cloudflare-access-structural-policy",
        artifacts: [],
        evidence: "tested",
      },
    ],
  });

  const result = checkAffiliateP0Complete(root);

  assert.equal(result.complete, false);
  assert.ok(
    result.errors.includes(
      "Affiliate / referral P0 manifest is missing required completed evidence: admin-affiliate-review-e2e",
    ),
  );
});

test("requires Cloudflare target-account metadata in apply evidence", async () => {
  const root = await makeRoot();
  await writeFile(
    path.join(root, "docs/evidence/cloudflare-admin-access/latest.json"),
    `${JSON.stringify({
      plan_log: validLogEvidence(),
      apply_log: validLogEvidence(),
    })}\n`,
  );
  await writeManifest(root, {
    status: "complete",
    blocking_items: [],
    completed_evidence: [
      {
        id: "admin-affiliate-review-e2e",
        artifacts: ["apps/admin/tests/e2e/affiliate-review.spec.ts"],
        evidence: "tested",
      },
    ],
  });

  const result = checkAffiliateP0Complete(root);

  assert.equal(result.complete, false);
  assert.ok(
    result.errors.includes(
      "Cloudflare admin Access evidence is missing valid target-account metadata.",
    ),
  );
});

test("accepts complete affiliate P0 manifest with admin review E2E evidence", async () => {
  const root = await makeRoot();
  await writeManifest(root, {
    status: "complete",
    blocking_items: [],
    completed_evidence: [
      {
        id: "admin-affiliate-review-e2e",
        artifacts: [
          "apps/admin/app/affiliate/commissions/page.tsx",
          "apps/admin/app/affiliate/payouts/page.tsx",
          "apps/admin/app/affiliate/actions.ts",
          "apps/admin/lib/affiliate-management-bff.ts",
          "apps/admin/tests/e2e/affiliate-review.spec.ts",
        ],
        evidence: "pnpm --filter @prophetica/admin test:e2e tests/e2e/affiliate-review.spec.ts => 2 passed",
      },
    ],
  });

  const result = checkAffiliateP0Complete(root);

  assert.equal(result.complete, true);
  assert.equal(result.completedEvidenceCount, 1);
  assert.deepEqual(result.errors, []);
});

test("finalizes the affiliate P0 manifest only after external evidence exists", async () => {
  const root = await makeRoot();
  await rm(path.join(root, "docs/evidence/cloudflare-admin-access/latest.json"), {
    force: true,
  });
  await writeManifest(root, {
    last_updated: "2026-06-29",
    status: "blocked",
    blocking_items: [
      {
        id: "cloudflare-access-apply-evidence",
        reason: "missing Cloudflare apply evidence",
      },
    ],
    completed_evidence: [
      {
        id: "admin-affiliate-review-e2e",
        artifacts: ["apps/admin/tests/e2e/affiliate-review.spec.ts"],
        evidence: "tested",
      },
    ],
  });

  const failed = spawnSync(process.execPath, [finalizeScript, root], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  assert.equal(failed.status, 1);
  assert.match(
    failed.stderr,
    /Missing Cloudflare admin Access apply evidence/,
  );
  const blockedManifest = JSON.parse(
    await readFile(path.join(root, "docs/affiliate-p0-evidence-manifest.json"), "utf8"),
  );
  assert.equal(blockedManifest.status, "blocked");

  await writeFile(
    path.join(root, "docs/evidence/cloudflare-admin-access/latest.json"),
    `${JSON.stringify({
      target_account: validTargetAccountEvidence(),
      plan_log: validLogEvidence(),
      apply_log: validLogEvidence(),
    })}\n`,
  );

  execFileSync(process.execPath, [finalizeScript, root], { cwd: repoRoot });

  const manifest = JSON.parse(
    await readFile(path.join(root, "docs/affiliate-p0-evidence-manifest.json"), "utf8"),
  );
  assert.equal(manifest.status, "complete");
  assert.deepEqual(manifest.blocking_items, []);
  assert.match(
    manifest.completed_evidence.at(-1).evidence,
    /finalize-affiliate-p0-complete\.mjs/,
  );
  assert.equal(checkAffiliateP0Complete(root).complete, true);
});

async function writeManifest(root, manifest) {
  await mkdir(path.join(root, "docs"), { recursive: true });
  await writeFile(
    path.join(root, "docs/affiliate-p0-evidence-manifest.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
}

function validLogEvidence() {
  return {
    sha256: "a".repeat(64),
    bytes: 1200,
    summary: "Apply complete",
  };
}

function validTargetAccountEvidence() {
  return {
    account_id_sha256: "b".repeat(64),
    account_id_suffix: "abcdef",
    admin_app_domain: "admin.example.com",
    allowed_email_domains_sha256: "c".repeat(64),
    breakglass_email_sha256: "d".repeat(64),
    internal_service_token_name: "prophetica-admin-access",
  };
}
