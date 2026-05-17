#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const repoRoot = resolve(import.meta.dirname, "../..");
const manifestPath = resolve(
  repoRoot,
  "docs/affiliate-p0-evidence-manifest.json",
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

const cloudflareEvidencePath = resolve(
  repoRoot,
  "docs/evidence/cloudflare-admin-access/latest.json",
);
const webauthnEvidencePath = resolve(
  repoRoot,
  "docs/evidence/webauthn-passkey/latest.json",
);
const publicSignupEvidencePath = resolve(
  repoRoot,
  "docs/evidence/public-signup-referral/latest.json",
);

if (!existsSync(cloudflareEvidencePath)) {
  console.error(
    "Missing Cloudflare admin Access apply evidence: docs/evidence/cloudflare-admin-access/latest.json",
  );
  process.exit(1);
}

const cloudflareEvidence = JSON.parse(readFileSync(cloudflareEvidencePath, "utf8"));
if (
  !isValidCloudflareLogEvidence(cloudflareEvidence.plan_log) ||
  !isValidCloudflareLogEvidence(cloudflareEvidence.apply_log)
) {
  console.error(
    "Cloudflare admin Access evidence is missing valid plan/apply hashes, byte counts, or success summaries.",
  );
  process.exit(1);
}

if (!existsSync(webauthnEvidencePath)) {
  console.error(
    "Missing WebAuthn / passkey browser evidence: docs/evidence/webauthn-passkey/latest.json",
  );
  process.exit(1);
}

const webauthnEvidence = JSON.parse(readFileSync(webauthnEvidencePath, "utf8"));
if (
  webauthnEvidence.registration_round_trip !== true ||
  webauthnEvidence.registration_replay_rejected !== true ||
  webauthnEvidence.assertion_round_trip !== true ||
  webauthnEvidence.assertion_replay_rejected !== true ||
  webauthnEvidence.wrong_origin_rejected !== true ||
  webauthnEvidence.unknown_credential_rejected !== true ||
  webauthnEvidence.revoked_credential_rejected !== true ||
  webauthnEvidence.fresh_hardware_mfa_session !== true ||
  webauthnEvidence.admin_step_up_fresh_accepts !== true ||
  webauthnEvidence.admin_step_up_stale_rejects !== true
) {
  console.error("WebAuthn / passkey evidence does not satisfy the P0 contract.");
  process.exit(1);
}

if (!existsSync(publicSignupEvidencePath)) {
  console.error(
    "Missing public signup referral evidence: docs/evidence/public-signup-referral/latest.json",
  );
  process.exit(1);
}

const publicSignupEvidence = JSON.parse(readFileSync(publicSignupEvidencePath, "utf8"));
if (
  publicSignupEvidence.signup_submit_path !== true ||
  publicSignupEvidence.referral_cookie_submitted !== true ||
  publicSignupEvidence.registration_endpoint_confirmed !== true ||
  publicSignupEvidence.durable_login_confirmed !== true ||
  publicSignupEvidence.referral_capture_confirmed !== true
) {
  console.error("Public signup referral evidence does not satisfy the P0 contract.");
  process.exit(1);
}

console.log(
  `Affiliate / referral P0 manifest is complete (${completedEvidence.length} evidence entries).`,
);

function isValidCloudflareLogEvidence(logEvidence) {
  return (
    logEvidence &&
    typeof logEvidence.sha256 === "string" &&
    /^[a-f0-9]{64}$/i.test(logEvidence.sha256) &&
    Number.isInteger(logEvidence.bytes) &&
    logEvidence.bytes > 0 &&
    typeof logEvidence.summary === "string" &&
    logEvidence.summary.trim().length > 0
  );
}
