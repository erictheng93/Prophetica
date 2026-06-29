#!/usr/bin/env node
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const defaultRepoRoot = resolve(import.meta.dirname, "../..");

const requiredCompletedEvidenceIds = ["admin-affiliate-review-e2e"];

export function checkAffiliateP0Complete(repoRoot = defaultRepoRoot) {
  const errors = [];
  const manifestPath = resolve(
    repoRoot,
    "docs/affiliate-p0-evidence-manifest.json",
  );
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  const blockers = Array.isArray(manifest.blocking_items)
    ? manifest.blocking_items
    : [];

  if (manifest.status !== "complete" || blockers.length > 0) {
    errors.push("Affiliate / referral P0 is not complete.");
    errors.push(`Manifest status: ${manifest.status ?? "unknown"}`);
    if (blockers.length > 0) {
      errors.push("Blocking items:");
      for (const blocker of blockers) {
        errors.push(`- ${blocker.id}: ${blocker.reason}`);
      }
    }
    return { complete: false, completedEvidenceCount: 0, errors };
  }

  const completedEvidence = Array.isArray(manifest.completed_evidence)
    ? manifest.completed_evidence
    : [];

  if (completedEvidence.length === 0) {
    errors.push("Affiliate / referral P0 manifest has no completed evidence.");
  }

  const completedEvidenceIds = new Set(
    completedEvidence
      .map((entry) => (typeof entry?.id === "string" ? entry.id : null))
      .filter(Boolean),
  );
  for (const requiredId of requiredCompletedEvidenceIds) {
    if (!completedEvidenceIds.has(requiredId)) {
      errors.push(
        `Affiliate / referral P0 manifest is missing required completed evidence: ${requiredId}`,
      );
    }
  }

  for (const entry of completedEvidence) {
    const artifacts = Array.isArray(entry?.artifacts) ? entry.artifacts : [];
    for (const artifact of artifacts) {
      if (typeof artifact !== "string" || artifact.trim().length === 0) {
        errors.push(
          `Affiliate / referral P0 completed evidence ${entry?.id ?? "unknown"} has an invalid artifact path.`,
        );
        continue;
      }
      if (!existsSync(resolve(repoRoot, artifact))) {
        errors.push(
          `Affiliate / referral P0 completed evidence ${entry?.id ?? "unknown"} references missing artifact: ${artifact}`,
        );
      }
    }
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
    errors.push(
      "Missing Cloudflare admin Access apply evidence: docs/evidence/cloudflare-admin-access/latest.json",
    );
  } else {
    const cloudflareEvidence = JSON.parse(
      readFileSync(cloudflareEvidencePath, "utf8"),
    );
    if (
      !isValidCloudflareLogEvidence(cloudflareEvidence.plan_log) ||
      !isValidCloudflareLogEvidence(cloudflareEvidence.apply_log)
    ) {
      errors.push(
        "Cloudflare admin Access evidence is missing valid plan/apply hashes, byte counts, or success summaries.",
      );
    }
    if (!isValidCloudflareTargetEvidence(cloudflareEvidence.target_account)) {
      errors.push(
        "Cloudflare admin Access evidence is missing valid target-account metadata.",
      );
    }
  }

  if (!existsSync(webauthnEvidencePath)) {
    errors.push(
      "Missing WebAuthn / passkey browser evidence: docs/evidence/webauthn-passkey/latest.json",
    );
  } else {
    const webauthnEvidence = JSON.parse(
      readFileSync(webauthnEvidencePath, "utf8"),
    );
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
      errors.push("WebAuthn / passkey evidence does not satisfy the P0 contract.");
    }
  }

  if (!existsSync(publicSignupEvidencePath)) {
    errors.push(
      "Missing public signup referral evidence: docs/evidence/public-signup-referral/latest.json",
    );
  } else {
    const publicSignupEvidence = JSON.parse(
      readFileSync(publicSignupEvidencePath, "utf8"),
    );
    if (
      publicSignupEvidence.signup_submit_path !== true ||
      publicSignupEvidence.referral_cookie_submitted !== true ||
      publicSignupEvidence.registration_endpoint_confirmed !== true ||
      publicSignupEvidence.durable_login_confirmed !== true ||
      publicSignupEvidence.referral_capture_confirmed !== true
    ) {
      errors.push("Public signup referral evidence does not satisfy the P0 contract.");
    }
  }

  return {
    complete: errors.length === 0,
    completedEvidenceCount: completedEvidence.length,
    errors,
  };
}

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

function isValidCloudflareTargetEvidence(targetEvidence) {
  return (
    targetEvidence &&
    typeof targetEvidence.account_id_sha256 === "string" &&
    /^[a-f0-9]{64}$/i.test(targetEvidence.account_id_sha256) &&
    typeof targetEvidence.account_id_suffix === "string" &&
    /^[A-Za-z0-9_-]{4,16}$/.test(targetEvidence.account_id_suffix) &&
    typeof targetEvidence.admin_app_domain === "string" &&
    targetEvidence.admin_app_domain.trim().length > 0 &&
    typeof targetEvidence.allowed_email_domains_sha256 === "string" &&
    /^[a-f0-9]{64}$/i.test(targetEvidence.allowed_email_domains_sha256) &&
    typeof targetEvidence.breakglass_email_sha256 === "string" &&
    /^[a-f0-9]{64}$/i.test(targetEvidence.breakglass_email_sha256) &&
    typeof targetEvidence.internal_service_token_name === "string" &&
    targetEvidence.internal_service_token_name.trim().length > 0
  );
}

function runCli() {
  const result = checkAffiliateP0Complete();
  if (!result.complete) {
    for (const error of result.errors) {
      console.error(error);
    }
    process.exit(1);
  }

  console.log(
    `Affiliate / referral P0 manifest is complete (${result.completedEvidenceCount} evidence entries).`,
  );
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  runCli();
}
